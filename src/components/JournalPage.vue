<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ArrowRight, Check, ChevronLeft, ChevronRight, CircleCheck, FileText, ImagePlus, LoaderCircle, MoreHorizontal, NotebookPen, Pencil, Plus, Trash2 } from '@lucide/vue'
import { api } from '../api'
import { addDays, dateKey, kindNames, parseDate, periodFor, shortDate, statisticsFor, typeNames, type DateRange, type Goal, type ImageUpload, type Note, type NoteImage, type NoteKind, type ReportType } from '../domain'
import { maxNoteImages, prepareImage } from '../images'
import { busy, dateGoals, dateNotes, dateStatistics, mutate, notify, selectedDate, view, workspace } from '../store'
import ModalShell from './ModalShell.vue'
import DateRangePicker from './DateRangePicker.vue'
import NoteImages from './NoteImages.vue'

const emit = defineEmits<{ generate: [type: ReportType, anchorDate: string, range?: DateRange] }>()
const filter = ref<NoteKind | 'all'>('all')
const visibleNotes = computed(() => dateNotes.value.filter(note => filter.value === 'all' || note.kind === filter.value))
const goalInput = ref('')
const editing = ref<Note | null>(null)
const editorOpen = ref(false)
const content = ref('')
const tag = ref('')
const kind = ref<NoteKind>('progress')
const attachedImages = ref<NoteImage[]>([])
const imageUrls = ref<Record<string, string>>({})
const imageUploads = ref<ImageUpload[]>([])
const preparingImages = ref(false)
const imageInput = ref<HTMLInputElement>()
let editorVersion = 0
const editGoal = ref<Goal | null>(null)
const editGoalTitle = ref('')
const deletion = ref<{ type: 'note' | 'goal'; id: string } | null>(null)
const goalMenu = ref('')
const today = dateKey(new Date())
const weekday = computed(() => new Intl.DateTimeFormat('zh-CN', { weekday: 'long' }).format(parseDate(selectedDate.value)))
const heading = computed(() => `${parseDate(selectedDate.value).getMonth() + 1}月${parseDate(selectedDate.value).getDate()}日`)
const reportType = ref<ReportType>('weekly')
const reportAnchor = ref(selectedDate.value)
const reportDates = ref<DateRange>(periodFor('weekly', selectedDate.value))
const customRange = ref(false)
const reportRangeOpen = ref(false)
const reportPeriod = computed(() => reportType.value === 'daily' ? periodFor('daily', reportAnchor.value) : reportDates.value)
const reportRange = computed(() => {
  const { start, end } = reportPeriod.value
  return start === end ? shortDate(start) : `${shortDate(start)} — ${shortDate(end, start.slice(0, 4) !== end.slice(0, 4))}`
})
const periodUnit = computed(() => ({ daily: '天', weekly: '周', monthly: '个月' })[reportType.value])
watch(selectedDate, date => {
  if (customRange.value && reportType.value !== 'daily') return
  reportAnchor.value = date
  reportDates.value = periodFor(reportType.value, date)
})
watch(reportType, type => {
  customRange.value = false
  reportAnchor.value = selectedDate.value
  reportDates.value = periodFor(type, selectedDate.value)
})
const weekStats = computed(() => {
  const { start, end } = periodFor('weekly', selectedDate.value)
  return statisticsFor(workspace.value.notes.filter(note => note.date >= start && note.date <= end), workspace.value.goals.filter(goal => goal.date >= start && goal.date <= end))
})
function shiftReportPeriod(amount: number) {
  const current = reportPeriod.value
  let nextRange: DateRange
  let nextAnchor: string
  if (reportType.value === 'monthly') {
    const shift = (value: string) => {
      const date = parseDate(value)
      const day = date.getDate()
      date.setDate(1)
      date.setMonth(date.getMonth() + amount)
      date.setDate(Math.min(day, new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate()))
      return dateKey(date)
    }
    nextAnchor = shift(current.start)
    nextRange = customRange.value ? { start: shift(current.start), end: shift(current.end) } : periodFor('monthly', nextAnchor)
  } else {
    const offset = amount * (reportType.value === 'weekly' ? 7 : 1)
    nextAnchor = addDays(reportAnchor.value, offset)
    nextRange = { start: addDays(current.start, offset), end: addDays(current.end, offset) }
  }
  if ([nextRange.start, nextRange.end].some(date => date < '1900-01-01' || date > '9999-12-31')) return notify('日期必须在 1900 至 9999 年之间')
  reportAnchor.value = nextAnchor
  reportDates.value = nextRange
}
function chooseReportRange(range: DateRange) {
  reportDates.value = range
  reportAnchor.value = range.start
  customRange.value = true
}

function openEditor(note?: Note) {
  editorVersion++
  editing.value = note || null
  content.value = note?.content || ''
  tag.value = note?.tag || ''
  kind.value = note?.kind || 'progress'
  attachedImages.value = [...(note?.images || [])]
  imageUrls.value = {}
  imageUploads.value = []
  preparingImages.value = false
  editorOpen.value = true
}
defineExpose({ openEditor })

function closeEditor() {
  if (busy.value) return
  editorVersion++
  editorOpen.value = false
  attachedImages.value = []
  imageUrls.value = {}
  imageUploads.value = []
}

async function attachFiles(files: File[]) {
  if (preparingImages.value || busy.value) return notify('请等待图片处理完成')
  if (attachedImages.value.length + files.length > maxNoteImages) return notify('每条记录最多添加 9 张图片')
  const version = editorVersion
  preparingImages.value = true
  try {
    for (const file of files) {
      const prepared = await prepareImage(file)
      if (version !== editorVersion) return
      attachedImages.value.push(prepared.image)
      imageUploads.value.push(prepared.upload)
      imageUrls.value[prepared.image.id] = prepared.url
    }
  } catch (error) { if (version === editorVersion) notify(error instanceof Error ? error.message : String(error)) }
  finally { if (version === editorVersion) preparingImages.value = false }
}

function uploadImages(event: Event) {
  const input = event.target as HTMLInputElement
  void attachFiles(Array.from(input.files || []))
  input.value = ''
}

function pasteImages(event: ClipboardEvent) {
  const files = Array.from(event.clipboardData?.items || []).filter(item => item.kind === 'file' && item.type.startsWith('image/')).map(item => item.getAsFile()).filter((file): file is File => !!file)
  if (!files.length) return
  event.preventDefault()
  const text = event.clipboardData?.getData('text/plain')
  if (text && event.target instanceof HTMLTextAreaElement) {
    event.target.setRangeText(text, event.target.selectionStart, event.target.selectionEnd, 'end')
    content.value = event.target.value.slice(0, 32000)
  }
  void attachFiles(files)
}

function removeImage(id: string) {
  attachedImages.value = attachedImages.value.filter(image => image.id !== id)
  imageUploads.value = imageUploads.value.filter(image => image.id !== id)
  delete imageUrls.value[id]
}

async function saveNote() {
  if (busy.value || preparingImages.value) return
  if (!content.value.trim() && !attachedImages.value.length) return notify('请填写记录内容或添加图片')
  const now = new Date().toISOString()
  const success = await mutate(() => api.saveNote({ id: editing.value?.id || crypto.randomUUID(), date: editing.value?.date || selectedDate.value, content: content.value.trim(), tag: tag.value.trim(), kind: kind.value, createdAt: editing.value?.createdAt || now, updatedAt: now, images: attachedImages.value }, imageUploads.value), '记录已保存')
  if (success) closeEditor()
}

async function addGoal() {
  if (!goalInput.value.trim() || busy.value) return
  const now = new Date().toISOString()
  const success = await mutate(() => api.saveGoal({ id: crypto.randomUUID(), date: selectedDate.value, title: goalInput.value.trim(), completed: false, createdAt: now, updatedAt: now }))
  if (success) goalInput.value = ''
}

async function toggleGoal(goal: Goal) {
  await mutate(() => api.saveGoal({ ...goal, completed: !goal.completed, updatedAt: new Date().toISOString() }))
}

async function saveGoalEdit() {
  if (!editGoal.value || !editGoalTitle.value.trim()) return
  const goal = { ...editGoal.value, title: editGoalTitle.value.trim(), updatedAt: new Date().toISOString() }
  if (await mutate(() => api.saveGoal(goal), '待办已更新')) editGoal.value = null
}

async function confirmDelete() {
  if (!deletion.value) return
  const { type, id } = deletion.value
  if (await mutate(() => type === 'note' ? api.deleteNote(id) : api.deleteGoal(id), '已删除')) deletion.value = null
}

async function loadExamples() { await mutate(() => api.loadExamples(), '已载入示例记录') }

function time(value: string) { return new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value)) }
</script>

<template>
  <div class="page-heading journal-page-heading">
    <div class="journal-title"><h1 :title="selectedDate">{{ heading }}</h1><span class="weekday">{{ weekday }}</span></div>
    <div class="heading-actions"><div class="date-controls"><button class="icon-button" aria-label="前一天" @click="selectedDate = addDays(selectedDate, -1)"><ChevronLeft :size="17" /></button><button class="text-button" @click="selectedDate = today">今天</button><button class="icon-button" aria-label="后一天" @click="selectedDate = addDays(selectedDate, 1)"><ChevronRight :size="17" /></button></div><button class="button primary" @click="openEditor()"><Plus :size="17" />写记录</button></div>
  </div>
  <div class="journal-layout">
    <section class="journal-column">
      <div class="section-heading notes-heading"><h2>工作记录 <span class="count">{{ dateNotes.length }}</span></h2><div class="filter-tabs" aria-label="记录分类"><button v-for="item in [{ value: 'all', label: '全部' }, { value: 'progress', label: '进展' }, { value: 'note', label: '备忘' }, { value: 'blocker', label: '问题' }]" :key="item.value" :class="{ active: filter === item.value }" :aria-pressed="filter === item.value" @click="filter = item.value as NoteKind | 'all'">{{ item.label }}</button></div></div>
      <button class="quick-capture" title="Ctrl + N" @click="openEditor()"><span class="capture-icon"><NotebookPen :size="22" :stroke-width="1.6" /></span><span>写下工作进展…</span><Plus :size="18" /></button>
      <div v-if="visibleNotes.length" class="note-timeline">
        <article v-for="note in visibleNotes" :key="note.id" class="note-row">
          <div class="note-time"><span>{{ time(note.createdAt) }}</span><i :class="note.kind" /></div>
          <div class="note-card"><div class="note-card-top"><div class="note-labels"><span class="kind-label" :class="note.kind">{{ kindNames[note.kind] }}</span><span v-if="note.tag" class="tag-label">{{ note.tag }}</span></div><div class="note-actions"><button class="icon-button small" aria-label="编辑记录" @click="openEditor(note)"><Pencil :size="14" /></button><button class="icon-button small" aria-label="删除记录" @click="deletion = { type: 'note', id: note.id }"><Trash2 :size="14" /></button></div></div><p v-if="note.content" class="note-content">{{ note.content }}</p><NoteImages v-if="note.images?.length" :images="note.images" /><div v-if="note.updatedAt !== note.createdAt" class="edited-label">已编辑</div></div>
        </article>
      </div>
      <div v-else class="empty-state journal-empty"><span class="empty-illustration"><NotebookPen :size="35" :stroke-width="1.5" /></span><h3>{{ dateNotes.length ? '暂无这类记录' : '暂无记录' }}</h3><button v-if="!workspace.notes.length && !workspace.goals.length" class="text-button example-button" :disabled="busy" @click="loadExamples">载入示例<ArrowRight :size="14" /></button></div>
    </section>
    <aside class="day-aside">
      <section class="panel goals-panel"><div class="section-heading"><h2>待办清单</h2><span class="subtle">{{ dateStatistics.completed }} / {{ dateStatistics.goals }}</span></div><div v-if="dateGoals.length" class="goal-progress"><div class="progress-track"><div :style="{ width: `${dateStatistics.completionRate}%` }" /></div><span>{{ dateStatistics.completionRate }}%</span></div>
        <div class="goal-list"><div v-for="goal in dateGoals" :key="goal.id" class="goal-row" :class="{ complete: goal.completed }"><button class="goal-check" :class="{ checked: goal.completed }" :aria-label="`${goal.completed ? '取消完成' : '完成待办'}：${goal.title}`" :aria-pressed="goal.completed" :disabled="busy" @click="toggleGoal(goal)"><Check v-if="goal.completed" :size="13" /></button><span>{{ goal.title }}</span><div class="goal-menu-wrap"><button class="icon-button small" :aria-label="`待办操作：${goal.title}`" @click="goalMenu = goalMenu === goal.id ? '' : goal.id"><MoreHorizontal :size="15" /></button><div v-if="goalMenu === goal.id" class="mini-menu"><button @click="editGoal = goal; editGoalTitle = goal.title; goalMenu = ''"><Pencil :size="13" />编辑</button><button @click="deletion = { type: 'goal', id: goal.id }; goalMenu = ''"><Trash2 :size="13" />删除</button></div></div></div></div>
        <p v-if="!dateGoals.length" class="empty-goals">暂无待办</p><form class="add-goal" @submit.prevent="addGoal"><Plus :size="17" /><input v-model="goalInput" aria-label="新增待办" placeholder="添加待办" maxlength="300" /><button v-if="goalInput.trim()" class="icon-button small" type="submit" aria-label="保存待办" :disabled="busy"><ArrowRight :size="16" /></button></form><div v-if="dateGoals.length && dateStatistics.completed === dateStatistics.goals" class="all-done"><CircleCheck :size="14" />今日待办已全部完成</div>
      </section>
      <section class="panel report-panel"><div class="section-heading"><h2>工作报告</h2></div><div class="segmented report-type-selector" aria-label="报告周期"><button v-for="type in (['daily', 'weekly', 'monthly'] as const)" :key="type" :class="{ active: reportType === type }" :aria-pressed="reportType === type" @click="reportType = type">{{ typeNames[type] }}</button></div><div class="report-range"><button class="icon-button small period-step-previous" :disabled="reportRangeOpen" :aria-label="`前一${periodUnit}`" @click="shiftReportPeriod(-1)"><ChevronLeft :size="16" /></button><span v-if="reportType === 'daily'">{{ reportRange }}</span><DateRangePicker v-else :key="reportType" :model-value="reportDates" compact @update:model-value="chooseReportRange" @open-change="reportRangeOpen = $event" /><button class="icon-button small period-step-next" :disabled="reportRangeOpen" :aria-label="`后一${periodUnit}`" @click="shiftReportPeriod(1)"><ChevronRight :size="16" /></button></div><button class="button primary generate-quick" :disabled="reportRangeOpen" @click="emit('generate', reportType, reportAnchor, reportType === 'daily' ? undefined : reportPeriod)"><FileText :size="16" />生成报告</button></section>
      <section class="panel week-panel"><div class="section-heading"><h2>本周统计</h2><button class="icon-button small" aria-label="查看统计概览" @click="view = 'insights'"><ArrowRight :size="16" /></button></div><div class="week-metrics"><div><strong>{{ weekStats.days }}</strong><span>记录天数</span></div><div><strong>{{ weekStats.notes }}</strong><span>工作记录</span></div><div><strong>{{ weekStats.completed }}</strong><span>完成待办</span></div></div></section>
    </aside>
  </div>
  <ModalShell v-if="editorOpen" :title="editing ? '编辑记录' : '写记录'" editor @close="closeEditor"><form class="modal-body note-editor-form" @paste="pasteImages" @submit.prevent="saveNote" @keydown.ctrl.enter.prevent="saveNote"><div class="form-date">{{ editing?.date || selectedDate }}</div><div class="segmented note-kind-selector"><button v-for="item in (['progress', 'note', 'blocker'] as const)" :key="item" type="button" :class="{ active: kind === item }" @click="kind = item">{{ kindNames[item] }}</button></div><label class="field-label" for="note-content">记录内容</label><textarea id="note-content" v-model="content" class="note-editor" rows="7" maxlength="32000" placeholder="写下工作进展、备忘或问题，可粘贴图片…" />
      <div class="image-attach-toolbar"><button class="text-button" type="button" :disabled="busy || preparingImages || attachedImages.length >= maxNoteImages" title="最多 9 张，单张不超过 20 MB" @click="imageInput?.click()"><LoaderCircle v-if="preparingImages" class="spin" :size="16" /><ImagePlus v-else :size="16" />{{ preparingImages ? '处理图片…' : '上传图片' }}</button><span v-if="attachedImages.length" class="subtle">{{ attachedImages.length }} / {{ maxNoteImages }}</span><input ref="imageInput" type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/bmp,image/avif" multiple hidden aria-label="上传记录图片" @change="uploadImages" /></div>
      <NoteImages v-if="attachedImages.length" :images="attachedImages" :urls="imageUrls" editable :disabled="busy || preparingImages" @remove="removeImage" /><label class="field-label" for="note-tag">标签 <span>选填</span></label><input id="note-tag" v-model="tag" placeholder="添加标签" maxlength="40" /><footer class="modal-footer"><span class="subtle">Ctrl + Enter 保存</span><button class="button primary" type="submit" :disabled="busy || preparingImages || (!content.trim() && !attachedImages.length)"><LoaderCircle v-if="busy" class="spin" :size="16" />保存记录</button></footer></form></ModalShell>
  <ModalShell v-if="editGoal" title="编辑待办" @close="editGoal = null"><form class="modal-body" @submit.prevent="saveGoalEdit"><label class="field-label" for="goal-title">待办内容</label><input id="goal-title" v-model="editGoalTitle" maxlength="300" required /><footer class="modal-footer"><button class="button secondary" type="button" @click="editGoal = null">取消</button><button class="button primary" type="submit" :disabled="busy || !editGoalTitle.trim()">保存待办</button></footer></form></ModalShell>
  <ModalShell v-if="deletion" title="删除记录" @close="deletion = null"><div class="modal-body"><p class="confirm-copy">删除后无法恢复，已生成的报告保留原有内容。</p><footer class="modal-footer"><button class="button secondary" @click="deletion = null">取消</button><button class="button danger" :disabled="busy" @click="confirmDelete">删除</button></footer></div></ModalShell>
</template>
