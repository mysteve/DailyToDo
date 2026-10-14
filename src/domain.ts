import { planComparisons, planTable, type PlanData, type PlanComparison } from './plans'
export type NoteKind = 'note' | 'progress' | 'blocker'
export type ReportType = 'daily' | 'weekly' | 'monthly'
export type ReportStyle = 'brief' | 'detailed' | 'review'
export type GenerationMode = 'local' | 'ai'

export interface NoteImage {
  id: string
  name: string
  mime: string
  width: number
  height: number
}

export interface ImageUpload {
  id: string
  data: string
}

export interface Note {
  id: string
  date: string
  content: string
  kind: NoteKind
  tag: string
  createdAt: string
  updatedAt: string
  images?: NoteImage[]
  goalId?: string
}

export type Priority = 'high' | 'normal' | 'low'

export interface Goal {
  id: string
  date: string
  title: string
  completed: boolean
  createdAt: string
  updatedAt: string
  /** 自由填写的项目名称，留空时回退到标题中的「项目：事项」写法 */
  project?: string
  priority?: Priority
  /** 预估耗时（分钟） */
  estimateMin?: number
  /** 实际耗时（分钟） */
  actualMin?: number
  /** 勾选完成的时间；旧数据没有该字段 */
  completedAt?: string
  /** 首次计划日期，顺延后保留原日期 */
  originDate?: string
  carryCount?: number
  /** 风险或未完成原因，仅在填写时进入报告 */
  risk?: string
  /** 插活：中途临时安排、不在原计划内的事项 */
  adHoc?: boolean
}

export const maxMinutes = 100000
export const priorityNames: Record<Priority, string> = { high: '高', normal: '中', low: '低' }

export interface Statistics {
  days: number
  notes: number
  goals: number
  completed: number
  blockers: number
  completionRate: number
  estimateMinutes?: number
  actualMinutes?: number
}

export interface ReportRequest {
  type: ReportType
  anchorDate: string
  startDate?: string
  endDate?: string
  style: ReportStyle
  mode: GenerationMode
  includeNotes: boolean
  includeGoals: boolean
  includeCommits?: boolean
  instruction: string
}

export interface DateRange {
  start: string
  end: string
}

export interface Report {
  id: string
  title: string
  type: ReportType
  style: ReportStyle
  mode: GenerationMode
  startDate: string
  endDate: string
  markdown: string
  statistics: Statistics
  createdAt: string
  model: string
  sources: { planComparisons?: PlanComparison[]; notes: Note[]; goals: Goal[]; commits?: Commit[] }
}

export interface Commit {
  date: string
  repository: string
  sha: string
  message: string
}

export type GitProvider = 'github' | 'gitlab'

export interface GitSettings {
  enabled: boolean
  provider: GitProvider
  gitlabUrl: string
  hasToken: boolean
}

export const defaultGitSettings: GitSettings = { enabled: false, provider: 'github', gitlabUrl: 'https://gitlab.com', hasToken: false }

export function gitTokenPage(git: Pick<GitSettings, 'provider' | 'gitlabUrl'>): string {
  if (git.provider === 'github') return 'https://github.com/settings/tokens/new?scopes=repo&description=riji-worklog'
  let url: URL
  try { url = new URL(git.gitlabUrl.trim()) } catch { throw new Error('请输入完整的 GitLab 地址') }
  const local = ['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)
  if (url.protocol !== 'https:' && !(url.protocol === 'http:' && local)) throw new Error('GitLab 地址请使用 HTTPS；本地服务可使用 HTTP')
  if (url.username || url.password || url.search || url.hash) throw new Error('GitLab 地址不能包含凭据、查询参数或片段')
  return `${url.origin}${url.pathname.replace(/\/+$/, '')}/-/user_settings/personal_access_tokens?name=riji-worklog&scopes=read_api`
}

export interface AiSettings {
  endpoint: string
  model: string
  hasApiKey: boolean
  thinking?: boolean
  reportTemplates?: ReportTemplates
  git?: GitSettings
}

export type ReportTemplates = Record<ReportType, string>

export const templateVariables = ['周计划进展对照', '标题', '日期年-月-日', '日期范围', '汇报摘要', '月度目标达成情况', '重点项目成果与实际影响', '风险与协调事项', '下阶段重点任务与交付', '未完成事项与原因', '项目完成情况', '项目计划', '范围', '概况', '已完成待办', '未完成待办', '每日待办', '耗时统计'] as const
export const defaultReportTemplates: ReportTemplates = {
  daily: '# 日报 {{日期年-月-日}}\n\n## 【重点工作与交付】\n{{项目完成情况}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【明日重点计划】\n{{项目计划}}',
  weekly: '# 周报\n\n{{日期范围}}\n\n## 【本周总体情况】\n{{汇报摘要}}\n\n## 周计划进展对照\n{{周计划进展对照}}\n\n## 【重点项目与节点进展】\n{{重点项目成果与实际影响}}\n\n## 【进度风险与需协调事项】\n{{风险与协调事项}}\n\n## 【下周重点计划】\n{{下阶段重点任务与交付}}',
  monthly: '# 月报\n\n{{日期范围}}\n\n## 【月度目标达成情况】\n{{月度目标达成情况}}\n\n## 【主要成果与业务价值】\n{{重点项目成果与实际影响}}\n\n## 【未完成事项与原因】\n{{未完成事项与原因}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【下月重点计划】\n{{下阶段重点任务与交付}}',
}

export function resolveReportTemplates(saved?: Partial<ReportTemplates>): ReportTemplates {
  const templates = { ...defaultReportTemplates, ...saved }
  const previousDaily = '# 日报 {{日期年-月-日}}\n\n## 【总体情况】\n{{汇报摘要}}\n\n## 【重点工作与交付】\n{{重点项目成果与实际影响}}\n\n## 【风险与需协调事项】\n{{风险与协调事项}}\n\n## 【明日重点计划】\n{{下阶段重点任务与交付}}'
  if (templates.daily === previousDaily) templates.daily = defaultReportTemplates.daily
  for (const [type, period, kind, date, plan] of [
    ['daily', '今日', '日报', '日期年-月-日', '明日计划'],
    ['weekly', '本周', '周报', '日期范围', '下周计划'],
    ['monthly', '本月', '月报', '日期范围', '下月计划'],
  ] as const) {
    const previous = `# {{标题}}\n\n{{范围}}\n\n## ${period}概况\n{{概况}}\n\n## ${period}完成\n{{已完成待办}}\n\n## 待继续推进\n{{未完成待办}}`
    const progress = `${kind}{{${date}}}完成任务进度和计划\n\n【完成情况】：\n{{项目完成情况}}\n\n【遇到问题】\n无\n\n【${plan}】\n{{项目计划}}`
    if (templates[type] === previous || templates[type] === progress || templates[type] === defaultReportTemplates[type].replace('## 周计划进展对照\n{{周计划进展对照}}\n\n', '')) templates[type] = defaultReportTemplates[type]
  }
  return templates
}

export function validateReportTemplates(templates: ReportTemplates) {
  for (const type of ['daily', 'weekly', 'monthly'] as const) {
    const value = templates[type]
    if (typeof value !== 'string' || !value.trim() || [...value].length > 10000) throw new Error(`${typeNames[type]}模板需填写，且不能超过 10000 字`)
  }
}

export interface Workspace extends PlanData {
  notes: Note[]
  goals: Goal[]
  reports: Report[]
  settings: AiSettings
  storagePath: string
}

export const typeNames: Record<ReportType, string> = { daily: '日报', weekly: '周报', monthly: '月报' }
export const styleNames: Record<ReportStyle, string> = { brief: '简洁汇报', detailed: '详细汇报', review: '个人复盘' }
export const kindNames: Record<NoteKind, string> = { note: '备忘', progress: '进展', blocker: '问题' }

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
}

export function parseDate(value: string): Date {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) throw new Error('请选择有效日期')
  const [year, month, day] = value.split('-').map(Number)
  const date = new Date(year!, month! - 1, day!, 12)
  if (dateKey(date) !== value) throw new Error('请选择有效日期')
  return date
}

export function addDays(value: string, days: number): string {
  const date = parseDate(value)
  date.setDate(date.getDate() + days)
  return dateKey(date)
}

export function periodFor(type: ReportType, anchor: string): { start: string; end: string } {
  const date = parseDate(anchor)
  if (type === 'daily') return { start: anchor, end: anchor }
  if (type === 'weekly') {
    const start = addDays(anchor, -((date.getDay() + 6) % 7))
    return { start, end: addDays(start, 6) }
  }
  return {
    start: dateKey(new Date(date.getFullYear(), date.getMonth(), 1, 12)),
    end: dateKey(new Date(date.getFullYear(), date.getMonth() + 1, 0, 12)),
  }
}

export function shortDate(value: string, year = false): string {
  const date = parseDate(value)
  return `${year ? `${date.getFullYear()}年` : ''}${date.getMonth() + 1}月${date.getDate()}日`
}

export function periodLabel(start: string, end: string): string {
  return start === end ? shortDate(start, true) : `${shortDate(start, true)} — ${shortDate(end, start.slice(0, 4) !== end.slice(0, 4))}`
}

export function requestPeriod(request: ReportRequest): DateRange {
  const anchor = parseDate(request.anchorDate)
  if (anchor.getFullYear() < 1900 || anchor.getFullYear() > 9999) throw new Error('日期必须在 1900 至 9999 年之间')
  let range: DateRange
  if (request.type === 'daily' || (request.startDate === undefined && request.endDate === undefined)) {
    range = periodFor(request.type, request.anchorDate)
  } else {
    if (!request.startDate || !request.endDate) throw new Error('请选择开始日期和结束日期')
    range = { start: request.startDate, end: request.endDate }
  }
  for (const value of [range.start, range.end]) {
    const year = parseDate(value).getFullYear()
    if (year < 1900 || year > 9999) throw new Error('日期必须在 1900 至 9999 年之间')
  }
  if (range.start > range.end) throw new Error('结束日期不能早于开始日期')
  return range
}

export function selectSources(workspace: Pick<Workspace, 'notes' | 'goals'>, request: ReportRequest) {
  const { start, end } = requestPeriod(request)
  return {
    notes: [] as Note[],
    goals: workspace.goals.filter(goal => goal.date <= end && (goal.date >= start || request.type === 'daily' && !goal.completed)),
  }
}

export function formatMinutes(minutes: number): string {
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  if (!hours) return `${rest} 分钟`
  return rest ? `${hours} 小时 ${rest} 分钟` : `${hours} 小时`
}

function timeSummary(goals: Goal[]): string | null {
  const recorded = goals.filter(goal => goal.actualMin !== undefined)
  if (!recorded.length) return null
  return `${recorded.length} 项待办记录了实际耗时，合计 ${formatMinutes(recorded.reduce((sum, goal) => sum + (goal.actualMin || 0), 0))}。`
}

export function statisticsFor(notes: Note[], goals: Goal[]): Statistics {
  const completed = goals.filter(goal => goal.completed).length
  return {
    estimateMinutes: goals.reduce((sum, goal) => sum + (goal.estimateMin || 0), 0),
    actualMinutes: goals.reduce((sum, goal) => sum + (goal.actualMin || 0), 0),
    days: new Set([...notes.map(note => note.date), ...goals.map(goal => goal.date)]).size,
    notes: notes.length,
    goals: goals.length,
    completed,
    blockers: notes.filter(note => note.kind === 'blocker').length,
    completionRate: goals.length ? Math.round(completed / goals.length * 100) : 0,
  }
}

export function noteSummary(note: Note): string {
  const images = note.images?.length || 0
  return [note.content, images ? `附图 ${images} 张` : ''].filter(Boolean).join(' · ')
}

function projectItems(goals: Goal[], detail?: { label: string; confirmation: string }, includeTime = true, singleLine = false): string {
  if (!goals.length) return '暂无'
  const projects = new Map<string, string[]>()
  for (const goal of goals) {
    const title = goal.title.trim()
    const spent = !includeTime || goal.actualMin === undefined ? '' : `（实际耗时 ${formatMinutes(goal.actualMin)}）`
    const explicit = goal.project?.trim()
    if (explicit) {
      const items = projects.get(explicit) || []
      items.push(title + spent)
      projects.set(explicit, items)
      continue
    }
    const labelled = title.match(/^(?:【([^】]*)】|\[([^\]]*)\])(.*)$/s)
    const separated = labelled ? null : title.match(/^([^：:]*)[：:](.*)$/s)
    const project = (labelled?.[1] ?? labelled?.[2] ?? separated?.[1])?.trim()
    const content = (labelled?.[3] ?? separated?.[2])?.trim()
    const grouped = project && content && !project.includes('\n') && !content.includes('\n')
    const name = grouped ? project : '待办事项'
    const items = projects.get(name) || []
    items.push((grouped ? content : title) + spent)
    projects.set(name, items)
  }
  const number = (index: number) => index < 20 ? String.fromCodePoint(0x2460 + index) : `${index + 1}、`
  return [...projects].map(([project, items], index) => {
    if (singleLine) return `${project}\n${items.map((content, item) => `${item + 1}、${content}`).join('\n')}`
    const lines = items.map((content, item) => `${number(item)}${detail ? `${detail.label}：` : ''}${content}`)
    if (detail) lines.push(`${number(items.length)}${detail.confirmation}：待确认`)
    return `${index + 1}、${project}：\n${lines.join('\n')}`
  }).join('\n')
}

/** 已记录的风险或未完成原因，按填写顺序编号；未填写时返回空数组 */
function recordedRisks(goals: Goal[]): string[] {
  const items: string[] = []
  for (const goal of goals) {
    const risk = goal.risk?.trim()
    if (!risk) continue
    items.push(`${items.length < 20 ? String.fromCodePoint(0x2460 + items.length) : `${items.length + 1}、`}${goal.title.trim()}：${risk}`)
  }
  return items
}

export function dailyReportTemplate(template: string): string {
  const hasPlan = /\{\{\s*(项目计划|下阶段重点任务与交付|未完成待办)\s*\}\}/.test(template)
  return hasPlan ? template : `${template.trimEnd()}\n\n## 【明日重点计划】\n{{项目计划}}`
}

export function localReport(workspace: Pick<Workspace, 'notes' | 'goals'> & PlanData & { settings?: AiSettings }, request: ReportRequest): Report {
  const { start, end } = requestPeriod(request)
  const { notes, goals } = selectSources(workspace, request)
  const comparisons = planComparisons(workspace, request)
  if (!goals.length && !comparisons.length) throw new Error('所选范围内没有可用于生成报告的待办事项或周计划')
  const stats = statisticsFor([], goals)
  const isDaily = request.type === 'daily'
  const title = `${shortDate(start)}${start !== end ? ` — ${shortDate(end)}` : ''}工作${typeNames[request.type]}`
  const list = (items: Goal[]) => items.length ? items.map(goal => `- [${goal.completed ? 'x' : ' '}] ${goal.project?.trim() ? `【${goal.project.trim()}】` : ''}${goal.title}${goal.adHoc ? '（插活）' : ''}${isDaily ? '' : `（${shortDate(goal.date)}${goal.actualMin === undefined ? '' : `，耗时 ${formatMinutes(goal.actualMin)}`}）`}`).join('\n') : '暂无'
  const daily = [...new Set(goals.map(goal => goal.date))].sort().map(date => `### ${shortDate(date)}\n\n${list(goals.filter(goal => goal.date === date))}`).join('\n\n')
  const completed = goals.filter(goal => goal.completed)
  const unfinished = goals.filter(goal => !goal.completed)
  const risks = recordedRisks(isDaily ? goals : unfinished)
  const period = { daily: '今日', weekly: '本周', monthly: '本月' }[request.type]
  const timeNote = isDaily ? null : timeSummary(goals)
  const summary = `${comparisons.length ? `周计划 ${comparisons.length} 项，已完成 ${comparisons.filter(r => r.current.status === 'completed').length} 项，阻塞 ${comparisons.filter(r => r.current.status === 'blocked').length} 项。\n` : ''}${period}共 ${stats.goals} 项待办，已完成 ${stats.completed} 项，待推进 ${unfinished.length} 项。${timeNote ? `\n${timeNote}` : ''}`
  const values: Record<string, string> = {
    周计划进展对照: planTable(comparisons),
    标题: title, 范围: periodLabel(start, end), 日期: end, 报告日期: end, '日期年-月-日': end,
    日期范围: start === end ? end : `${start} — ${end}`,
    汇报摘要: summary,
    月度目标达成情况: `${summary}\n月度目标、验收标准及达成情况：待确认`,
    重点项目成果与实际影响: projectItems(completed, isDaily ? undefined : { label: '成果或进度', confirmation: '实际影响' }, !isDaily, isDaily),
    风险与协调事项: isDaily ? risks.join('\n') || '无' : risks.length ? `风险、进度影响及需协调事项：\n${risks.join('\n')}${risks.length < unfinished.length ? `\n其余 ${unfinished.length - risks.length} 项未记录风险或问题：待确认` : ''}` : '风险、进度影响及需协调事项：待确认',
    下阶段重点任务与交付: isDaily ? unfinished.map((goal, i) => `${i + 1}、${goal.title}`).join('\n') || '暂无' : projectItems(unfinished, { label: '重点任务', confirmation: '预期交付及完成时间' }),
    未完成事项与原因: unfinished.length ? `待推进事项：\n${projectItems(unfinished, undefined, !isDaily)}\n\n未完成原因：${risks.length ? `\n${risks.join('\n')}` : '待确认'}` : '所选范围内无未完成待办。',
    项目完成情况: projectItems(completed, undefined, !isDaily, isDaily),
    项目计划: isDaily ? unfinished.map((goal, i) => `${i + 1}、${goal.title}`).join('\n') || '暂无' : projectItems(unfinished),
    概况: `${stats.days} 天，共 ${stats.goals} 项待办，已完成 ${stats.completed} 项，完成率 ${stats.completionRate}%。${timeNote ? `\n${timeNote}` : ''}`,
    耗时统计: isDaily ? '' : timeNote ?? '未记录耗时',
    已完成待办: list(goals.filter(goal => goal.completed)),
    未完成待办: isDaily ? unfinished.map((goal, i) => `${i + 1}、${goal.title}`).join('\n') || '暂无' : list(unfinished), 每日待办: daily,
  }
  if (comparisons.length) {
    const blocked = comparisons.filter(r => r.current.status === 'blocked').map(r => `${r.current.title}：${r.current.blocker}`)
    if (blocked.length) values['风险与协调事项'] = [...blocked, ...risks].join('\n')
  }
  const templates = resolveReportTemplates(workspace.settings?.reportTemplates)
  validateReportTemplates(templates)
  let template = isDaily ? dailyReportTemplate(templates.daily) : templates[request.type]
  if (!isDaily && request.style === 'detailed' && !template.includes('{{每日待办}}')) template += '\n\n## 每日待办\n{{每日待办}}'
  if (comparisons.length && !template.includes('{{周计划进展对照}}')) template += '\n\n## 周计划进展对照\n{{周计划进展对照}}'
  const markdown = template.replace(/\{\{([^{}]*)\}\}/g, (_, key: string) => Object.hasOwn(values, key.trim()) ? values[key.trim()]! : `待填写：${key.trim() || '自定义字段'}`)
  return { id: crypto.randomUUID(), title, type: request.type, style: request.style, mode: 'local', startDate: start, endDate: end, markdown, statistics: stats, createdAt: new Date().toISOString(), model: '', sources: structuredClone({ notes, goals, planComparisons: comparisons }) }
}
