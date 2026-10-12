<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { CheckCircle2, Database, Download, ExternalLink, FileText, GitBranch, KeyRound, LoaderCircle, Monitor, RefreshCw, Sparkles, Upload } from '@lucide/vue'
import { api, desktop } from '../api'
import { busy, mutate, notify, workspace } from '../store'
import ModalShell from './ModalShell.vue'
import { defaultGitSettings, defaultReportTemplates, templateVariables, typeNames, validateReportTemplates, type GitProvider, type GitSettings, type ReportTemplates, type ReportType } from '../domain'

const emit = defineEmits<{ close: [] }>()
const endpoint = ref(workspace.value.settings.endpoint)
const model = ref(workspace.value.settings.model)
const thinking = ref(workspace.value.settings.thinking === true)
const templateType = ref<ReportType>('daily')
const reportTemplates = ref<ReportTemplates>({ ...defaultReportTemplates, ...workspace.value.settings.reportTemplates })
const templateEditor = ref<HTMLTextAreaElement>()

async function insertVariable(variable: string) {
  const editor = templateEditor.value
  const value = reportTemplates.value[templateType.value]
  const start = editor?.selectionStart ?? value.length
  const end = editor?.selectionEnd ?? start
  const token = `{{${variable}}}`
  if (value.length - (end - start) + token.length > 10000) return
  reportTemplates.value[templateType.value] = value.slice(0, start) + token + value.slice(end)
  await nextTick()
  editor?.focus()
  editor?.setSelectionRange(start + token.length, start + token.length)
}

onMounted(async () => {
  await nextTick()
  templateEditor.value?.setSelectionRange(0, 0)
  if (templateEditor.value) templateEditor.value.scrollTop = 0
})
const apiKey = ref('')
const clearKey = ref(false)
const savedGit = workspace.value.settings.git
const gitEnabled = ref(savedGit?.enabled ?? defaultGitSettings.enabled)
const gitProvider = ref<GitProvider>(savedGit?.provider ?? defaultGitSettings.provider)
const gitlabUrl = ref(savedGit?.gitlabUrl ?? defaultGitSettings.gitlabUrl)
const gitToken = ref('')
const clearGitToken = ref(false)
const gitTesting = ref(false)
const gitResult = ref('')
const gitError = ref('')
const error = ref('')
const testing = ref(false)
const testResult = ref('')
const backupBusy = ref(false)
const models = ref<string[]>([])
const modelsRefreshing = ref(false)
const modelsError = ref('')
let mounted = true

watch([endpoint, apiKey, clearKey], () => {
  models.value = []
  modelsError.value = ''
  testResult.value = ''
})
onBeforeUnmount(() => { mounted = false })

async function refreshModels() {
  modelsError.value = ''
  if (!/^https?:\/\//.test(endpoint.value.trim())) { modelsError.value = '请输入完整接口地址'; return }
  modelsRefreshing.value = true
  try {
    const result = await api.listModels(endpoint.value.trim(), apiKey.value, clearKey.value)
    if (mounted) { models.value = result; notify(`已获取 ${result.length} 个模型`) }
  } catch (err) {
    if (mounted) modelsError.value = String(err)
  } finally { modelsRefreshing.value = false }
}

watch([gitProvider, gitlabUrl, gitToken, clearGitToken], () => {
  gitResult.value = ''
  gitError.value = ''
})

function gitSettings(): GitSettings {
  return { enabled: gitEnabled.value, provider: gitProvider.value, gitlabUrl: gitlabUrl.value.trim(), hasToken: Boolean(workspace.value.settings.git?.hasToken) }
}

function tokenKept() {
  const saved = workspace.value.settings.git
  return Boolean(saved?.hasToken && !clearGitToken.value && saved.provider === gitProvider.value && (gitProvider.value === 'github' || saved.gitlabUrl === gitlabUrl.value.trim().replace(/\/+$/, '')))
}

async function openTokenPage() {
  gitError.value = ''
  try { await api.openGitTokenPage(gitSettings()) } catch (err) { gitError.value = err instanceof Error ? err.message : String(err) }
}

async function testGit() {
  gitError.value = ''
  gitResult.value = ''
  if (!gitToken.value.trim() && !tokenKept()) { gitError.value = '请先填写访问令牌'; return }
  gitTesting.value = true
  try { gitResult.value = await api.testGit(gitSettings(), gitToken.value) } catch (err) { gitError.value = String(err) }
  finally { gitTesting.value = false }
}

async function save(close = true) {
  error.value = ''
  if (!/^https?:\/\//.test(endpoint.value.trim())) { error.value = '请输入完整接口地址'; return false }
  try { validateReportTemplates(reportTemplates.value) } catch (err) { error.value = err instanceof Error ? err.message : String(err); return false }
  const success = await mutate(() => api.saveSettings({ endpoint: endpoint.value.trim(), model: model.value.trim(), hasApiKey: workspace.value.settings.hasApiKey, thinking: thinking.value, reportTemplates: { ...reportTemplates.value }, git: gitSettings() }, apiKey.value, clearKey.value, gitToken.value, clearGitToken.value))
  if (success) { apiKey.value = ''; clearKey.value = false; gitToken.value = ''; clearGitToken.value = false; if (close) { notify('设置已保存'); emit('close') } }
  return success
}

async function test() {
  if (!model.value.trim()) { error.value = '请填写模型名称'; return }
  if (!await save(false)) return
  testing.value = true
  testResult.value = ''
  try { testResult.value = await api.testAi() } catch (err) { error.value = String(err) }
  finally { testing.value = false }
}

async function backup(importing: boolean) {
  backupBusy.value = true
  try {
    if (importing) {
      let imported = false
      await mutate(async () => { imported = await api.importBackup(); if (imported) notify('备份已合并，已有记录保持原样') })
      if (imported) reportTemplates.value = { ...defaultReportTemplates, ...workspace.value.settings.reportTemplates }
    }
    else if (await api.exportBackup()) notify('备份已导出')
  } catch (err) { notify(String(err)) }
  finally { backupBusy.value = false }
}
</script>

<template>
  <ModalShell title="数据与设置" @close="emit('close')">
    <form class="modal-body settings-body" @submit.prevent="save()">
      <div class="settings-section-heading"><FileText :size="18" /><h3>报告模板</h3></div>
      <div class="segmented template-tabs"><button v-for="type in (['daily', 'weekly', 'monthly'] as const)" :key="type" type="button" :class="{ active: templateType === type }" :aria-pressed="templateType === type" @click="templateType = type">{{ typeNames[type] }}</button></div>
      <label class="field-label" for="report-template">{{ typeNames[templateType] }}模板 <button type="button" class="text-button" @click="reportTemplates[templateType] = defaultReportTemplates[templateType]">恢复默认</button></label>
      <textarea id="report-template" ref="templateEditor" v-model="reportTemplates[templateType]" rows="10" maxlength="10000" autocomplete="off" required />
      <p class="field-help">周报可包含周计划及上周对照；日报、月报使用待办事项。</p>
      <details class="template-help"><summary>插入模板变量</summary><div class="template-variables"><button v-for="variable in templateVariables" :key="variable" type="button" class="button secondary compact" @click="insertVariable(variable)">{{ variable }}</button></div><p class="field-help">支持自定义占位符。AI 根据待办填写，缺少依据时标记待确认；本地整理将自定义字段标记为待填写。</p></details>
      <div class="settings-divider" />
      <details class="settings-details"><summary>AI 服务</summary>
      <div class="settings-section-heading"><Sparkles :size="18" /><h3>AI 服务</h3><span class="status-pill" :class="{ configured: workspace.settings.model }">{{ workspace.settings.model ? '已配置' : '未配置' }}</span></div>
      <div class="service-presets"><button class="text-button" type="button" :disabled="modelsRefreshing || testing" @click="endpoint = 'https://api.openai.com/v1'">兼容 OpenAI</button><span>·</span><button class="text-button" type="button" :disabled="modelsRefreshing || testing" @click="endpoint = 'http://127.0.0.1:11434/v1'">本地 Ollama</button></div>
      <label class="field-label" for="ai-endpoint">接口地址</label>
      <input id="ai-endpoint" v-model="endpoint" type="url" maxlength="1000" autocomplete="off" placeholder="https://your-service.com/v1" :disabled="modelsRefreshing || testing" required />
      <label class="field-label" for="ai-model">模型名称</label>
      <div class="model-input-row">
        <input id="ai-model" v-model="model" list="ai-models" maxlength="200" autocomplete="off" placeholder="选择或填写模型名称" :disabled="testing" />
        <datalist id="ai-models"><option v-for="id in models" :key="id" :value="id" /></datalist>
        <button type="button" class="button secondary compact" aria-label="刷新模型列表" :disabled="!desktop || modelsRefreshing || testing || busy" @click="refreshModels"><LoaderCircle v-if="modelsRefreshing" class="spin" :size="14" /><RefreshCw v-else :size="14" />{{ modelsRefreshing ? '刷新中…' : '刷新' }}</button>
      </div>
      <p v-if="modelsError" class="inline-error" role="alert">{{ modelsError }}</p>
      <label class="field-label" for="ai-key">API Key <span>本地服务可留空</span></label>
      <div class="key-input"><KeyRound :size="16" /><input id="ai-key" v-model="apiKey" type="password" maxlength="8192" autocomplete="off" :disabled="!desktop || clearKey || modelsRefreshing || testing" :placeholder="workspace.settings.hasApiKey ? '已保存密钥 · 留空保持' : '填写你的 API Key'" /></div>
      <p class="field-help">{{ desktop ? '密钥保存在系统凭据库。修改接口地址时，请重新填写密钥。' : '浏览器仅供界面预览，AI 服务请在桌面应用中配置。' }}</p>
      <label v-if="workspace.settings.hasApiKey" class="checkbox-label"><input v-model="clearKey" type="checkbox" :disabled="modelsRefreshing || testing" />清除已保存的密钥</label>
      <div class="connection-row"><button type="button" class="button secondary compact" :disabled="!desktop || testing || modelsRefreshing || busy" @click="test"><LoaderCircle v-if="testing" class="spin" :size="14" /><Monitor v-else :size="14" />{{ testing ? '测试中…' : '保存并测试连接' }}</button><span v-if="testResult" class="connection-success"><CheckCircle2 :size="14" />{{ testResult }}</span></div>
      <label class="checkbox-label"><input v-model="thinking" type="checkbox" :disabled="testing || modelsRefreshing" />使用思考模式（默认关闭）</label>
      <p class="field-help">仅 DeepSeek 接口支持该参数。关闭后模型直接输出报告，速度更快，也不会因思维链占用输出长度而提示超出限制。</p>
      </details>
      <div class="settings-divider" />
      <details class="settings-details"><summary>代码提交</summary>
      <div class="settings-section-heading"><GitBranch :size="18" /><h3>代码提交</h3><span class="status-pill" :class="{ configured: workspace.settings.git?.enabled && workspace.settings.git?.hasToken }">{{ workspace.settings.git?.enabled && workspace.settings.git?.hasToken ? '已启用' : '未启用' }}</span></div>
      <label class="checkbox-label"><input v-model="gitEnabled" type="checkbox" />允许生成 AI 报告时附带我的 Git 提交记录</label>
      <label class="field-label" for="git-provider">代码平台</label>
      <select id="git-provider" v-model="gitProvider" :disabled="gitTesting"><option value="github">GitHub</option><option value="gitlab">GitLab</option></select>
      <template v-if="gitProvider === 'gitlab'"><label class="field-label" for="git-gitlab-url">GitLab 地址 <span>自建服务请修改</span></label><input id="git-gitlab-url" v-model="gitlabUrl" type="url" maxlength="1000" autocomplete="off" placeholder="https://gitlab.com" :disabled="gitTesting" /></template>
      <label class="field-label" for="git-token">访问令牌 <span>{{ gitProvider === 'github' ? '需要 repo 读取权限' : '需要 read_api 权限' }}</span></label>
      <div class="key-input"><KeyRound :size="16" /><input id="git-token" v-model="gitToken" type="password" maxlength="8192" autocomplete="off" :disabled="!desktop || clearGitToken || gitTesting" :placeholder="tokenKept() ? '已保存令牌 · 留空保持' : '填写你的访问令牌'" /></div>
      <p class="field-help">{{ desktop ? '令牌保存在系统凭据库，只用于读取你本人的提交。切换平台或 GitLab 地址时，请重新填写令牌。' : '浏览器仅供界面预览，Git 提交请在桌面应用中配置。' }}</p>
      <label v-if="workspace.settings.git?.hasToken" class="checkbox-label"><input v-model="clearGitToken" type="checkbox" :disabled="gitTesting" />清除已保存的令牌</label>
      <div class="connection-row"><button type="button" class="button secondary compact" :disabled="gitTesting" @click="openTokenPage"><ExternalLink :size="14" />获取令牌</button><button type="button" class="button secondary compact" :disabled="!desktop || gitTesting || busy" @click="testGit"><LoaderCircle v-if="gitTesting" class="spin" :size="14" /><Monitor v-else :size="14" />{{ gitTesting ? '测试中…' : '测试连接' }}</button><span v-if="gitResult" class="connection-success"><CheckCircle2 :size="14" />{{ gitResult }}</span></div>
      <p v-if="gitError" class="inline-error" role="alert">{{ gitError }}</p>
      </details>
      <p v-if="error" class="inline-error" role="alert">{{ error }}</p>
      <div class="settings-divider" /><details class="settings-details"><summary>本地数据</summary><div class="settings-section-heading"><Database :size="18" /><h3>本地数据</h3></div><div class="storage-path">{{ workspace.storagePath }}</div><p class="field-help">备份包含记录、图片、待办、报告和模板，不包含 AI 密钥。</p><div class="backup-actions"><button type="button" class="button secondary compact" :disabled="backupBusy || busy" @click="backup(false)"><Download :size="15" />导出备份</button><button type="button" class="button secondary compact" :disabled="backupBusy || busy" @click="backup(true)"><Upload :size="15" />导入备份</button></div></details><footer class="modal-footer"><span class="subtle">日迹 0.1.0</span><button class="button primary" type="submit" :disabled="busy || testing || modelsRefreshing"><LoaderCircle v-if="busy" class="spin" :size="15" />保存设置</button></footer></form></ModalShell>
</template>
