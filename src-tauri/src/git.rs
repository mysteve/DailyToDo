use crate::models::*;
use chrono::{DateTime, Local, NaiveDate, SecondsFormat, TimeZone};
use reqwest::{Client, Url};
use serde_json::Value;
use std::collections::HashSet;
use std::time::Duration;

const MAX_COMMITS: usize = 500;
const MAX_PROJECTS: usize = 100;
const TOO_MANY: &str = "所选范围内的提交过多，请缩小日期范围";

pub fn credential() -> Result<keyring::Entry, String> {
    keyring::Entry::new("com.riji.worklog", "git-access-token")
        .map_err(|_| "无法访问系统凭据存储".into())
}

pub fn read_token() -> Result<Option<String>, String> {
    match credential()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(_) => Err("无法读取系统保存的 Git 令牌".into()),
    }
}

pub fn clear_token() -> Result<(), String> {
    match credential()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(_) => Err("无法清除已保存的 Git 令牌".into()),
    }
}

fn is_loopback(url: &Url) -> bool {
    matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    )
}

pub fn gitlab_base(value: &str) -> Result<String, String> {
    let url = Url::parse(value.trim()).map_err(|_| "请输入完整的 GitLab 地址".to_string())?;
    if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback(&url)) {
        return Err("GitLab 地址请使用 HTTPS；本地服务可使用 HTTP".into());
    }
    if url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("GitLab 地址不能包含凭据、查询参数或片段".into());
    }
    Ok(url.as_str().trim_end_matches('/').to_string())
}

pub fn normalize(settings: &mut GitSettings) -> Result<(), String> {
    match gitlab_base(&settings.gitlab_url) {
        Ok(url) => settings.gitlab_url = url,
        Err(error) if settings.provider == GitProvider::Gitlab => return Err(error),
        Err(_) => settings.gitlab_url = GitSettings::default().gitlab_url,
    }
    Ok(())
}

pub fn token_page(settings: &GitSettings) -> Result<String, String> {
    Ok(match settings.provider {
        GitProvider::Github => {
            "https://github.com/settings/tokens/new?scopes=repo&description=riji-worklog".into()
        }
        GitProvider::Gitlab => format!(
            "{}/-/user_settings/personal_access_tokens?name=riji-worklog&scopes=read_api",
            gitlab_base(&settings.gitlab_url)?
        ),
    })
}

#[cfg(target_os = "windows")]
fn browser_command(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("rundll32");
    command.args(["url.dll,FileProtocolHandler", url]);
    command
}

#[cfg(target_os = "macos")]
fn browser_command(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(url);
    command
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn browser_command(url: &str) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(url);
    command
}

pub fn open_token_page(settings: &GitSettings) -> Result<(), String> {
    let url = token_page(settings)?;
    browser_command(&url)
        .spawn()
        .map(|_| ())
        .map_err(|_| format!("无法打开浏览器，请手动访问：{url}"))
}

struct ApiError {
    status: Option<u16>,
    message: String,
}

impl From<ApiError> for String {
    fn from(error: ApiError) -> Self {
        error.message
    }
}

struct Api {
    client: Client,
    base: String,
    token: String,
    provider: GitProvider,
}

impl Api {
    fn new(settings: &GitSettings, token: &str) -> Result<Self, String> {
        let base = match settings.provider {
            GitProvider::Github => "https://api.github.com".to_string(),
            GitProvider::Gitlab => format!("{}/api/v4", gitlab_base(&settings.gitlab_url)?),
        };
        let mut builder = Client::builder()
            .user_agent("riji-worklog")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(15))
            .redirect(reqwest::redirect::Policy::none());
        if Url::parse(&base).ok().is_some_and(|url| is_loopback(&url)) {
            builder = builder.no_proxy();
        }
        Ok(Self {
            client: builder.build().map_err(|_| "无法初始化 Git 连接")?,
            base,
            token: token.trim().to_string(),
            provider: settings.provider,
        })
    }

    async fn get(&self, path: &str, query: &[(&str, String)]) -> Result<Value, ApiError> {
        let mut request = self.client.get(format!("{}{path}", self.base)).query(query);
        request = match self.provider {
            GitProvider::Github => request
                .bearer_auth(&self.token)
                .header("Accept", "application/vnd.github+json"),
            GitProvider::Gitlab => request.header("PRIVATE-TOKEN", &self.token),
        };
        let response = request.send().await.map_err(|error| ApiError {
            status: None,
            message: if error.is_timeout() {
                "Git 请求超时，请稍后重试".into()
            } else {
                "无法连接 Git 服务，请检查地址和网络".into()
            },
        })?;
        let status = response.status().as_u16();
        if !response.status().is_success() {
            return Err(ApiError {
                status: Some(status),
                message: match status {
                    401 => "Git 令牌无效或已过期，请重新获取".into(),
                    403 => "Git 令牌权限不足或请求受限，请检查令牌权限".into(),
                    404 => "Git 服务地址或资源不存在，请检查配置".into(),
                    429 => "Git 服务请求受限，请稍后重试".into(),
                    _ => format!("Git 服务返回错误（HTTP {status}）"),
                },
            });
        }
        response.json().await.map_err(|_| ApiError {
            status: Some(status),
            message: "Git 响应格式不正确".into(),
        })
    }
}

struct Identity {
    login: String,
    names: HashSet<String>,
    emails: HashSet<String>,
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
}

async fn identity(api: &Api) -> Result<Identity, String> {
    let user = api.get("/user", &[]).await?;
    let key = match api.provider {
        GitProvider::Github => "login",
        GitProvider::Gitlab => "username",
    };
    let login = text(&user, key).ok_or("无法识别令牌所属账号")?.to_string();
    let names = ["name", key]
        .iter()
        .filter_map(|key| text(&user, key))
        .map(str::to_lowercase)
        .collect();
    let emails = ["email", "commit_email", "public_email"]
        .iter()
        .filter_map(|key| text(&user, key))
        .map(str::to_lowercase)
        .collect();
    Ok(Identity {
        login,
        names,
        emails,
    })
}

pub async fn test(settings: &GitSettings, token: &str) -> Result<String, String> {
    let api = Api::new(settings, token)?;
    let me = identity(&api).await?;
    let provider = match settings.provider {
        GitProvider::Github => "GitHub",
        GitProvider::Gitlab => "GitLab",
    };
    Ok(format!("连接成功 · {provider} {}", me.login))
}

fn bounds(start: &str, end: &str) -> Result<(String, String), String> {
    let convert = |date: &str, hour, minute, second| -> Result<String, String> {
        let day = NaiveDate::parse_from_str(date, "%Y-%m-%d").map_err(|_| "报告日期不正确")?;
        let time = day
            .and_hms_opt(hour, minute, second)
            .ok_or("报告日期不正确")?;
        Local
            .from_local_datetime(&time)
            .earliest()
            .map(|time| time.to_rfc3339_opts(SecondsFormat::Secs, false))
            .ok_or_else(|| "报告日期不正确".to_string())
    };
    Ok((convert(start, 0, 0, 0)?, convert(end, 23, 59, 59)?))
}

fn local_date(value: &str) -> Option<String> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Local).format("%Y-%m-%d").to_string())
}

fn subject(message: &str) -> String {
    message
        .lines()
        .next()
        .unwrap_or_default()
        .trim()
        .chars()
        .take(200)
        .collect()
}

fn short_sha(sha: &str) -> String {
    sha.chars().take(8).collect()
}

fn is_merge(item: &Value, key: &str) -> bool {
    item.get(key)
        .and_then(Value::as_array)
        .is_some_and(|parents| parents.len() > 1)
}

fn parse_github(payload: &Value) -> Vec<Commit> {
    payload
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter(|item| !is_merge(item, "parents"))
                .filter_map(|item| {
                    let commit = item.get("commit")?;
                    Some(Commit {
                        date: local_date(text(commit.get("author")?, "date")?)?,
                        repository: text(item.get("repository")?, "full_name")?.to_string(),
                        sha: short_sha(text(item, "sha")?),
                        message: subject(text(commit, "message")?),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_gitlab(payload: &Value, repository: &str, me: &Identity) -> Vec<Commit> {
    payload
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| !is_merge(item, "parent_ids"))
                .filter(|item| {
                    text(item, "author_email")
                        .is_some_and(|email| me.emails.contains(&email.to_lowercase()))
                        || text(item, "author_name")
                            .is_some_and(|name| me.names.contains(&name.to_lowercase()))
                })
                .filter_map(|item| {
                    Some(Commit {
                        date: local_date(
                            text(item, "authored_date").or(text(item, "created_at"))?,
                        )?,
                        repository: repository.to_string(),
                        sha: short_sha(text(item, "id")?),
                        message: subject(text(item, "title").or(text(item, "message"))?),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

async fn github_commits(api: &Api, from: &str, to: &str) -> Result<Vec<Commit>, String> {
    let me = identity(api).await?;
    let query = format!("author:{} author-date:{from}..{to}", me.login);
    let mut commits = Vec::new();
    for page in 1..=6 {
        let payload = api
            .get(
                "/search/commits",
                &[
                    ("q", query.clone()),
                    ("sort", "author-date".into()),
                    ("order", "asc".into()),
                    ("per_page", "100".into()),
                    ("page", page.to_string()),
                ],
            )
            .await?;
        let total = payload
            .get("total_count")
            .and_then(Value::as_u64)
            .unwrap_or_default() as usize;
        if total > MAX_COMMITS {
            return Err(TOO_MANY.into());
        }
        let rows = payload
            .get("items")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        commits.extend(parse_github(&payload));
        if rows < 100 {
            break;
        }
    }
    Ok(commits)
}

async fn gitlab_commits(api: &Api, from: &str, to: &str) -> Result<Vec<Commit>, String> {
    let me = identity(api).await?;
    let projects = api
        .get(
            "/projects",
            &[
                ("membership", "true".into()),
                ("simple", "true".into()),
                ("order_by", "last_activity_at".into()),
                ("last_activity_after", from.to_string()),
                ("per_page", MAX_PROJECTS.to_string()),
            ],
        )
        .await?;
    let projects = projects.as_array().ok_or("GitLab 项目列表格式不正确")?;
    if projects.len() >= MAX_PROJECTS {
        return Err("所选范围内有活动的项目过多，请缩小日期范围".into());
    }
    let mut commits = Vec::new();
    for project in projects {
        let (Some(id), Some(name)) = (
            project.get("id").and_then(Value::as_u64),
            text(project, "path_with_namespace"),
        ) else {
            continue;
        };
        for page in 1..=5 {
            let payload = match api
                .get(
                    &format!("/projects/{id}/repository/commits"),
                    &[
                        ("since", from.to_string()),
                        ("until", to.to_string()),
                        ("all", "true".into()),
                        ("per_page", "100".into()),
                        ("page", page.to_string()),
                    ],
                )
                .await
            {
                Ok(payload) => payload,
                Err(ApiError {
                    status: Some(403 | 404),
                    ..
                }) => break,
                Err(error) => return Err(error.message),
            };
            let rows = payload.as_array().map_or(0, Vec::len);
            commits.extend(parse_gitlab(&payload, name, &me));
            if commits.len() > MAX_COMMITS {
                return Err(TOO_MANY.into());
            }
            if rows < 100 {
                break;
            }
        }
    }
    Ok(commits)
}

pub async fn fetch_commits(
    settings: &GitSettings,
    token: &str,
    start: &str,
    end: &str,
) -> Result<Vec<Commit>, String> {
    let (from, to) = bounds(start, end)?;
    let api = Api::new(settings, token)?;
    let mut commits = match settings.provider {
        GitProvider::Github => github_commits(&api, &from, &to).await?,
        GitProvider::Gitlab => gitlab_commits(&api, &from, &to).await?,
    };
    let mut seen = HashSet::new();
    commits.retain(|commit| seen.insert((commit.repository.clone(), commit.sha.clone())));
    if commits.len() > MAX_COMMITS {
        return Err(TOO_MANY.into());
    }
    commits.sort_by(|a, b| (&a.date, &a.repository).cmp(&(&b.date, &b.repository)));
    Ok(commits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gitlab_address_requires_https_and_rejects_credentials() {
        assert_eq!(
            gitlab_base("https://gitlab.example.com/").unwrap(),
            "https://gitlab.example.com"
        );
        assert_eq!(
            gitlab_base("http://127.0.0.1:8080/gitlab/").unwrap(),
            "http://127.0.0.1:8080/gitlab"
        );
        assert!(gitlab_base("http://gitlab.example.com").is_err());
        assert!(gitlab_base("https://user:secret@gitlab.example.com").is_err());
        assert!(gitlab_base("https://gitlab.example.com?token=1").is_err());
    }

    #[test]
    fn token_pages_follow_the_selected_provider() {
        let mut settings = GitSettings::default();
        assert!(token_page(&settings)
            .unwrap()
            .starts_with("https://github.com/settings/tokens/new"));
        settings.provider = GitProvider::Gitlab;
        settings.gitlab_url = "https://git.example.com/".into();
        assert_eq!(
            token_page(&settings).unwrap(),
            "https://git.example.com/-/user_settings/personal_access_tokens?name=riji-worklog&scopes=read_api"
        );
        settings.gitlab_url = "http://git.example.com".into();
        assert!(token_page(&settings).is_err());
    }

    #[test]
    fn github_results_keep_subjects_only_and_skip_merges() {
        let payload = json!({"items": [
            {"sha": "0123456789abcdef", "parents": [{}], "repository": {"full_name": "me/app"}, "commit": {"message": "修复登录\n\n详细正文不应发送", "author": {"date": "2026-10-09T10:00:00+08:00"}}},
            {"sha": "ffffffffffff", "parents": [{}, {}], "repository": {"full_name": "me/app"}, "commit": {"message": "Merge branch", "author": {"date": "2026-10-09T11:00:00+08:00"}}},
            {"sha": "bad", "commit": {"message": "缺少仓库"}}
        ]});
        let commits = parse_github(&payload);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "修复登录");
        assert_eq!(commits[0].sha, "01234567");
        assert_eq!(commits[0].repository, "me/app");
    }

    #[test]
    fn gitlab_results_only_include_the_token_owner() {
        let me = Identity {
            login: "me".into(),
            names: HashSet::from(["me".to_string(), "张三".to_string()]),
            emails: HashSet::from(["me@example.com".to_string()]),
        };
        let payload = json!([
            {"id": "aaaaaaaaaaaa", "title": "新增导出", "author_name": "Someone", "author_email": "ME@example.com", "authored_date": "2026-10-09T10:00:00+08:00", "parent_ids": ["x"]},
            {"id": "bbbbbbbbbbbb", "title": "他人提交", "author_name": "Other", "author_email": "other@example.com", "authored_date": "2026-10-09T10:00:00+08:00", "parent_ids": ["x"]},
            {"id": "cccccccccccc", "title": "Merge", "author_name": "张三", "author_email": "me@example.com", "authored_date": "2026-10-09T10:00:00+08:00", "parent_ids": ["x", "y"]}
        ]);
        let commits = parse_gitlab(&payload, "team/app", &me);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].message, "新增导出");
        assert_eq!(commits[0].repository, "team/app");
    }

    #[test]
    fn date_bounds_cover_the_whole_range() {
        let (from, to) = bounds("2026-10-01", "2026-10-09").unwrap();
        assert!(from.starts_with("2026-10-01T00:00:00"));
        assert!(to.starts_with("2026-10-09T23:59:59"));
        assert!(bounds("2026-02-30", "2026-03-01").is_err());
    }
}
