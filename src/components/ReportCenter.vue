<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowRight, Check, ChevronDown, Copy, Download, FileText, FolderOpen, LoaderCircle, Plus, Sparkles, Trash2 } from '@lucide/vue'
import { planComparisons, fullWeeks, planStatusNames } from '../plans'
import { api, desktop } from '../api'
import { kindNames, noteSummary, periodFor, periodLabel, requestPeriod, selectSources, shortDate, statisticsFor, styleNames, typeNames, type DateRange, type Report, type ReportRequest, type ReportType } from '../domain'
import { mutate, notify, refresh, selectedDate, view, workspace } from '../store'
import ModalShell from './ModalShell.vue'
import NoteImages from './NoteImages.vue'
import MarkdownView from './MarkdownView.vue'
import DateRangePicker from './DateRangePicker.vue'

const props = defineProps<{ generatorOpen: boolean }>()
const emit = defineEmits<{ closeGenerator: []; openGenerator: []; settings: [] }>()
const filter = ref<ReportType | 'all'>('all')
const selectedId = ref('')
const filteredReports = computed(() => workspace.value.reports.filter(report => filter.value === 'all' || report.type === filter.value))
const selectedReport = computed(() => filteredReports.value.find(report => report.id === selectedId.value) || filteredReports.value[0])
const request = ref<ReportRequest>({ type: 'weekly', anchorDate: selectedDate.value, style: 'brief', mode: 'local', includeNotes: false, includeGoals: true, includeCommits: false, instruction: '' })
const generating = ref(false)
const generationError = ref('')
const deletion = ref<Report | null>(null)
const copied = ref(false)
const sourcesOpen = ref(false)
const rangeOpen = ref(false)
const available = computed(() => { try { return selectSources(workspace.value, request.value) } catch { return { notes: [], goals: [] } } })
const statistics = computed(() => statisticsFor(available.value.notes, available.value.goals))
const period = computed(() => {
  try { return requestPeriod(request.value) } catch { return null }
})
const periodError = computed(() => { try { requestPeriod(request.value); return '' } catch (error) { return error instanceof Error ? error.message : String(error) } })
const chosenRange = computed({
  get: () => period.value || periodFor(request.value.type, selectedDate.value),
  set: (range: DateRange) => { request.value = { ...request.value, anchorDate: range.start, startDate: range.start, endDate: range.end } },
})
const comparisons = computed(() => { try { return planComparisons(workspace.value, request.value) } catch { return [] } })
const partialWeek = computed(() => request.value.type === 'weekly' && period.value && !fullWeeks(period.value.start, period.value.end).length)
const validSource = computed(() => Boolean(period.value && (available.value.goals.length || comparisons.value.length)))
const gitReady = computed(() => Boolean(workspace.value.settings.git?.enabled && workspace.value.settings.git.hasToken))
const gitName = computed(() => workspace.value.settings.git?.provider === 'gitlab' ? 'GitLab' : 'GitHub')
const aiHost = computed(() => { try { return new URL(workspace.value.settings.endpoint).host } catch { return '' } })
function configureRequest(type: ReportType | undefined, anchorDate: string, range?: DateRange) {
  const nextType = type || request.value.type
  const dates = range || periodFor(nextType, anchorDate)
  request.value = { ...request.value, type: nextType, anchorDate, startDate: nextType === 'daily' ? undefined : dates.start, endDate: nextType === 'daily' ? undefined : dates.end }
}
function chooseType(type: ReportType) {
  if (request.value.type === type) return
  configureRequest(type, request.value.anchorDate)
}
defineExpose({ configureRequest })

watch(() => props.generatorOpen, open => {
  if (open) { generationError.value = ''; request.value.mode = desktop && workspace.value.settings.model ? 'ai' : 'local'; request.value.includeCommits = gitReady.value }
})
watch(selectedReport, () => { sourcesOpen.value = false; copied.value = false })

async function generate() {
  if (generating.value || rangeOpen.value || !validSource.value) return
  generating.value = true
  generationError.value = ''
  try {
    const report = await api.generate({ ...request.value, includeCommits: request.value.mode === 'ai' && gitReady.value && Boolean(request.value.includeCommits), instruction: request.value.mode === 'ai' ? request.value.instruction : '' })
    await refresh()
    filter.value = 'all'
    selectedId.value = report.id
    view.value = 'reports'
    emit('closeGenerator')
    notify('报告已生成并保存在本地')
  } catch (error) { generationError.value = error instanceof Error ? error.message : String(error) }
  finally { generating.value = false }
}

async function copyReport() {
  if (!selectedReport.value) return
  try { await navigator.clipboard.writeText(selectedReport.value.markdown); copied.value = true; setTimeout(() => { copied.value = false }, 2000) }
  catch { notify('无法写入剪贴板，请导出报告后复制') }
}

async function exportReport() {
  if (!selectedReport.value) return
  try { if (await api.exportReport(selectedReport.value)) notify('报告已导出') } catch (error) { notify(String(error)) }
}

async function removeReport() {
  if (deletion.value && await mutate(() => api.deleteReport(deletion.value!.id), '报告已删除')) deletion.value = null
}
</script>

<template>
  <div class="page-heading"><h1>工作报告</h1><button class="button primary" @click="emit('openGenerator')"><Sparkles :size="17" />生成报告</button></div>
  <div class="report-toolbar"><div class="filter-tabs report-filters"><button v-for="item in [{ value: 'all', label: '全部报告' }, { value: 'daily', label: '日报' }, { value: 'weekly', label: '周报' }, { value: 'monthly', label: '月报' }]" :key="item.value" :class="{ active: filter === item.value }" @click="filter = item.value as ReportType | 'all'">{{ item.label }}</button></div><span class="subtle">{{ filteredReports.length }} 份报告</span></div>
  <div v-if="filteredReports.length" class="report-workspace"><aside class="report-list"><button v-for="report in filteredReports" :key="report.id" class="report-list-item" :class="{ active: report.id === selectedReport?.id }" @click="selectedId = report.id"><span class="report-list-top"><span class="report-type-label">{{ typeNames[report.type] }}</span><Sparkles v-if="report.mode === 'ai'" :size="14" /></span><strong>{{ shortDate(report.startDate) }}{{ report.startDate !== report.endDate ? ` — ${shortDate(report.endDate)}` : '' }}</strong><span class="subtle">{{ styleNames[report.style] }} · {{ report.statistics.goals }} 项待办</span><span class="report-created">{{ new Date(report.createdAt).toLocaleDateString('zh-CN') }} 生成</span></button></aside>
    <section v-if="selectedReport" class="report-document"><div class="report-document-toolbar"><span class="document-mode"><Sparkles v-if="selectedReport.mode === 'ai'" :size="14" /><FileText v-else :size="14" />{{ selectedReport.mode === 'ai' ? selectedReport.model : '本地整理' }}</span><div class="document-actions"><button class="icon-button" :aria-label="copied ? '已复制报告' : '复制报告'" @click="copyReport"><Check v-if="copied" :size="17" /><Copy v-else :size="17" /></button><button class="icon-button" aria-label="导出 Markdown" @click="exportReport"><Download :size="17" /></button><button class="icon-button" aria-label="删除报告" @click="deletion = selectedReport"><Trash2 :size="17" /></button></div></div><div class="document-content"><div class="document-period">{{ periodLabel(selectedReport.startDate, selectedReport.endDate) }}</div><div class="document-statistics"><span>{{ selectedReport.statistics.days }} 天待办</span><span>{{ selectedReport.statistics.completed }} / {{ selectedReport.statistics.goals }} 待办完成</span></div><MarkdownView :content="selectedReport.markdown" /><div class="source-footer"><button class="text-button" @click="sourcesOpen = !sourcesOpen"><FolderOpen :size="15" />{{ sourcesOpen ? '收起来源记录' : '查看来源记录' }}<ChevronDown :size="14" :class="{ rotated: sourcesOpen }" /></button><div v-if="sourcesOpen" class="source-preview"><div v-for="note in selectedReport.sources?.notes || []" :key="note.id" class="source-note"><p><span>{{ note.date }} · {{ kindNames[note.kind] }}</span>{{ noteSummary(note) }}</p><NoteImages v-if="note.images?.length" :images="note.images" /></div><p v-for="goal in selectedReport.sources?.goals || []" :key="goal.id"><span>{{ goal.date }} · {{ goal.completed ? '已完成待办' : '未完成待办' }}</span>{{ goal.title }}</p><p v-for="row in selectedReport.sources?.planComparisons || []" :key="row.current.planId + row.current.weekStart"><span>{{ row.current.weekStart }} · 周计划 · {{ planStatusNames[row.current.status] }}</span>{{ row.current.title }} · {{ row.current.progress || '尚未更新' }}<template v-if="row.current.blocker"> · 阻塞：{{ row.current.blocker }}</template></p><p v-for="commit in selectedReport.sources?.commits || []" :key="commit.repository + commit.sha"><span>{{ commit.date }} · {{ commit.repository }} · {{ commit.sha }}</span>{{ commit.message }}</p><p v-if="!selectedReport.sources?.notes?.length && !selectedReport.sources?.goals?.length && !selectedReport.sources?.commits?.length && !selectedReport.sources?.planComparisons?.length" class="subtle">此报告没有保存来源快照</p></div></div></div></section>
  </div>
  <div v-else class="empty-state reports-empty"><span class="empty-illustration"><FileText :size="38" :stroke-width="1.5" /></span><h3>{{ filter === 'all' ? '暂无报告' : `暂无${typeNames[filter]}` }}</h3><button class="button primary" @click="emit('openGenerator')"><Plus :size="16" />生成报告</button></div>

  <ModalShell v-if="generatorOpen" title="生成工作报告" wide @close="emit('closeGenerator')"><form class="modal-body generator-body" @submit.prevent="generate"><div class="generator-types"><button v-for="type in (['daily', 'weekly', 'monthly'] as const)" :key="type" type="button" :class="{ active: request.type === type }" :aria-pressed="request.type === type" @click="chooseType(type)"><FileText :size="18" /><strong>{{ typeNames[type] }}</strong><Check v-if="request.type === type" class="type-check" :size="14" /></button></div>
      <template v-if="request.type === 'daily'"><label for="report-date" class="field-label">报告日期</label><div class="period-field"><input id="report-date" v-model="request.anchorDate" type="date" min="1900-01-01" max="9999-12-31" required /></div></template><template v-else><div class="field-label">时间范围</div><DateRangePicker :key="request.type" v-model="chosenRange" @open-change="rangeOpen = $event" /></template>
      <label for="report-style" class="field-label">报告风格</label><select id="report-style" v-model="request.style"><option value="brief">简洁汇报</option><option value="detailed">详细汇报</option><option value="review">个人复盘</option></select>
      <p v-if="partialWeek" class="field-help">周计划对照需要完整自然周；当前范围仅使用待办。</p><div class="source-summary"><span v-if="comparisons.length">{{ comparisons.length }} 条周计划</span><span>{{ statistics.days }} 天待办</span><span>{{ statistics.goals }} 项待办</span><span>已完成 {{ statistics.completed }} 项</span></div><details class="sources-detail"><summary>查看报告来源<ChevronDown :size="14" /></summary><div class="source-preview"><p v-for="row in comparisons" :key="row.current.planId + row.current.weekStart"><span>{{ row.current.weekStart }} · {{ planStatusNames[row.current.status] }}</span>{{ row.current.title }}</p><p v-for="goal in available.goals" :key="goal.id"><span>{{ goal.date }} · {{ goal.completed ? '已完成' : '未完成' }}</span>{{ goal.title }}</p><p v-if="!validSource" class="subtle">所选范围内暂无待办或周计划</p></div></details>
      <div class="field-label">生成方式</div><div class="segmented mode-selector"><button type="button" :class="{ active: request.mode === 'local' }" @click="request.mode = 'local'"><FileText :size="15" />本地整理</button><button type="button" :class="{ active: request.mode === 'ai' }" @click="request.mode = 'ai'"><Sparkles :size="15" />AI 生成</button></div>
      <template v-if="request.mode === 'ai'"><div v-if="!desktop || !workspace.settings.model" class="inline-notice"><span>{{ !desktop ? 'AI 生成需在桌面应用中使用' : '先配置 AI 服务，再生成报告' }}</span><button v-if="desktop" type="button" class="text-button" @click="emit('settings')">去配置<ArrowRight :size="13" /></button></div><template v-else><p class="ai-disclosure">将所选待办{{ comparisons.length ? '、周计划及上周对照' : '' }}{{ gitReady && request.includeCommits ? '和提交标题' : '' }}发送至 {{ aiHost }} · {{ workspace.settings.model }}</p><label v-if="gitReady" class="checkbox-label"><input v-model="request.includeCommits" type="checkbox" />附带我在 {{ gitName }} 上的提交记录，仅发送提交标题、仓库、日期和短哈希，不含代码</label><label for="report-instruction" class="field-label">补充要求 <span>选填</span></label><textarea id="report-instruction" v-model="request.instruction" rows="2" maxlength="2000" autocomplete="off" /></template></template>
      <p v-if="periodError" class="inline-error" role="alert">{{ periodError }}</p><p v-else-if="!validSource" class="inline-error">所选范围内没有待办事项，请调整日期。</p><p v-if="generationError" class="inline-error" role="alert">{{ generationError }}</p><footer class="modal-footer"><button class="button primary" type="submit" :disabled="generating || rangeOpen || !validSource || (request.mode === 'ai' && (!desktop || !workspace.settings.model))"><LoaderCircle v-if="generating" class="spin" :size="16" /><Sparkles v-else :size="16" />{{ generating ? '正在生成…' : '生成报告' }}</button></footer></form></ModalShell>
  <ModalShell v-if="deletion" title="删除报告" @close="deletion = null"><div class="modal-body"><p class="confirm-copy">删除这份报告？原始手记与工作待办会保留。</p><footer class="modal-footer"><button class="button secondary" @click="deletion = null">取消</button><button class="button danger" @click="removeReport">删除报告</button></footer></div></ModalShell>
</template>
