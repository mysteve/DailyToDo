<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { CalendarArrowDown, Check, Clock, ImagePlus, LoaderCircle, MoreHorizontal, NotebookPen, Pencil, Plus, Trash2 } from '@lucide/vue'
import { api } from '../api'
import { backlog, carryOver, minutesInput, parseMinutes, projectNames, toggled } from '../analytics'
import { addDays, formatMinutes, kindNames, parseDate, priorityNames, shortDate, type Goal, type Priority, type ImageUpload, type Note, type NoteImage, type NoteKind } from '../domain'
import { maxNoteImages, prepareImage } from '../images'
import { busy, dateGoals, dateNotes, dateStatistics, mutate, notify, selectedDate, workspace } from '../store'
import ModalShell from './ModalShell.vue'
import NoteImages from './NoteImages.vue'

const filter = ref<NoteKind | 'all'>('all')
const visibleNotes = computed(() => dateNotes.value.filter(note => filter.value === 'all' || note.kind === filter.value))
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
const editGoalProject = ref('')
const editGoalPriority = ref<Priority | ''>('')
const editGoalEstimate = ref('')
const editGoalActual = ref('')
const creatingGoal = ref(false)
const editGoalRisk = ref('')
const editGoalAdHoc = ref(false)
const carryTarget = ref<{ goal: Goal; toDate: string } | null>(null)
const carryRisk = ref('')
const projects = computed(() => projectNames(workspace.value.goals))
const overdue = computed(() => backlog(workspace.value.goals, selectedDate.value))
const deletion = ref<{ type: 'note' | 'goal'; id: string } | null>(null)
const goalMenu = ref('')
const weekday = computed(() => new Intl.DateTimeFormat('zh-CN', { weekday: 'long' }).format(parseDate(selectedDate.value)))
const heading = computed(() => `${parseDate(selectedDate.value).getMonth() + 1}月${parseDate(selectedDate.value).getDate()}日`)

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

function addGoal() {
  if (busy.value) return
  const now = new Date().toISOString()
  openGoalEdit({ id: crypto.randomUUID(), date: selectedDate.value, title: '', completed: false, createdAt: now, updatedAt: now }, true)
}

async function toggleGoal(goal: Goal) {
  await mutate(() => api.saveGoal(toggled(goal)))
}

function openGoalEdit(goal: Goal, creating = false) {
  creatingGoal.value = creating
  editGoal.value = goal
  editGoalTitle.value = goal.title
  editGoalProject.value = goal.project || ''
  editGoalPriority.value = goal.priority || ''
  editGoalEstimate.value = minutesInput(goal.estimateMin)
  editGoalActual.value = minutesInput(goal.actualMin)
  editGoalRisk.value = goal.risk || ''
  editGoalAdHoc.value = goal.adHoc === true
  goalMenu.value = ''
}

async function saveGoalEdit() {
  if (!editGoal.value || !editGoalTitle.value.trim()) return
  let estimateMin: number | undefined
  let actualMin: number | undefined
  try {
    estimateMin = parseMinutes(editGoalEstimate.value)
    actualMin = parseMinutes(editGoalActual.value)
  } catch (error) { return notify(error instanceof Error ? error.message : String(error)) }
  const goal: Goal = { ...editGoal.value, title: editGoalTitle.value.trim(), updatedAt: new Date().toISOString() }
  const optional = { project: editGoalProject.value.trim() || undefined, priority: editGoalPriority.value || undefined, estimateMin, actualMin, risk: editGoalRisk.value.trim() || undefined, adHoc: editGoalAdHoc.value || undefined }
  for (const [key, value] of Object.entries(optional)) {
    if (value === undefined) delete (goal as unknown as Record<string, unknown>)[key]
    else (goal as unknown as Record<string, unknown>)[key] = value
  }
  const creating = creatingGoal.value
  if (await mutate(() => api.saveGoal(goal), creating ? '待办已添加' : '待办已更新')) {
    editGoal.value = null
  }
}

watch(editGoal, goal => { if (!goal) creatingGoal.value = false })

async function carryGoals(goals: Goal[], toDate: string) {
  const moved = carryOver(goals, toDate)
  if (!moved.length) return
  const batches = Array.from({ length: Math.ceil(moved.length / 500) }, (_, index) => moved.slice(index * 500, index * 500 + 500))
  await mutate(async () => { for (const batch of batches) await api.saveGoals(batch) }, `已顺延 ${moved.length} 项待办到 ${shortDate(toDate)}`)
  goalMenu.value = ''
}

function openCarry(goal: Goal, toDate = addDays(goal.date, 1)) {
  carryTarget.value = { goal, toDate }
  carryRisk.value = goal.risk || ''
  goalMenu.value = ''
}

async function confirmCarry() {
  const target = carryTarget.value
  if (!target) return
  const moved = carryOver([target.goal], target.toDate, { risk: carryRisk.value.trim() || undefined })
  if (!moved.length) return
  if (await mutate(() => api.saveGoals(moved), `已顺延到 ${shortDate(target.toDate)}`)) carryTarget.value = null
}

function goalMeta(goal: Goal): string[] {
  const items: string[] = []
  if (goal.actualMin !== undefined || goal.estimateMin !== undefined) items.push(goal.actualMin !== undefined ? `耗时 ${formatMinutes(goal.actualMin)}${goal.estimateMin ? ` / 预估 ${formatMinutes(goal.estimateMin)}` : ''}` : `预估 ${formatMinutes(goal.estimateMin!)}`)
  if (goal.carryCount) items.push(`已顺延 ${goal.carryCount} 次`)
  return items
}

async function confirmDelete() {
  if (!deletion.value) return
  const { type, id } = deletion.value
  if (await mutate(() => type === 'note' ? api.deleteNote(id) : api.deleteGoal(id), '已删除')) deletion.value = null
}

function time(value: string) { return new Intl.DateTimeFormat('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }).format(new Date(value)) }
</script>

<template>
  <div class="page-heading journal-page-heading">
    <div class="journal-title"><h1 :title="selectedDate">{{ heading }}</h1><span class="weekday">{{ weekday }}</span></div>
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
      <div v-else class="empty-state journal-empty"><span class="empty-illustration"><NotebookPen :size="35" :stroke-width="1.5" /></span><h3>{{ dateNotes.length ? '暂无这类记录' : '暂无记录' }}</h3></div>
    </section>
    <aside class="day-aside">
      <section class="panel goals-panel"><div class="section-heading"><h2>待办清单</h2><span class="subtle">{{ dateStatistics.completed }} / {{ dateStatistics.goals }}</span></div><div v-if="dateGoals.length" class="goal-progress"><div class="progress-track"><div :style="{ width: `${dateStatistics.completionRate}%` }" /></div><span>{{ dateStatistics.completionRate }}%</span></div>
        <div class="goal-list"><div v-for="goal in dateGoals" :key="goal.id" class="goal-row" :class="{ complete: goal.completed }"><button class="goal-check" :class="{ checked: goal.completed }" :aria-label="`${goal.completed ? '取消完成' : '完成待办'}：${goal.title}`" :aria-pressed="goal.completed" :disabled="busy" @click="toggleGoal(goal)"><Check v-if="goal.completed" :size="13" /></button><span class="goal-body"><span class="goal-title"><i v-if="goal.priority && goal.priority !== 'normal'" class="priority-dot" :class="goal.priority" :title="`优先级：${priorityNames[goal.priority]}`" />{{ goal.title }}<em v-if="goal.adHoc" class="adhoc-chip" title="插活：中途临时安排">插活</em></span><span v-if="goal.risk" class="goal-risk">风险／原因：{{ goal.risk }}</span><span v-if="goal.project || goalMeta(goal).length" class="goal-meta"><em v-if="goal.project" class="project-chip">{{ goal.project }}</em><span v-for="item in goalMeta(goal)" :key="item">{{ item }}</span></span></span><div class="goal-menu-wrap"><button class="icon-button small" :aria-label="`待办操作：${goal.title}`" @click="goalMenu = goalMenu === goal.id ? '' : goal.id"><MoreHorizontal :size="15" /></button><div v-if="goalMenu === goal.id" class="mini-menu"><button @click="openGoalEdit(goal)"><Pencil :size="13" />编辑</button><button v-if="!goal.completed" @click="openCarry(goal)"><CalendarArrowDown :size="13" />顺延到次日</button><button @click="deletion = { type: 'goal', id: goal.id }; goalMenu = ''"><Trash2 :size="13" />删除</button></div></div></div></div>
        <p v-if="!dateGoals.length" class="empty-goals">暂无待办</p><button class="add-goal" type="button" :disabled="busy" @click="addGoal"><Plus :size="17" /><span>添加待办</span></button>
        <datalist id="goal-projects"><option v-for="name in projects" :key="name" :value="name" /></datalist>
        <div v-if="overdue.length" class="carry-banner"><span><Clock :size="14" />往日有 {{ overdue.length }} 项未完成</span><button class="text-button" :disabled="busy" @click="carryGoals(overdue.map(item => item.goal), selectedDate)">顺延到这天</button></div>
      </section>
    </aside>
  </div>
  <ModalShell v-if="editorOpen" :title="editing ? '编辑记录' : '写记录'" editor @close="closeEditor"><form class="modal-body note-editor-form" @paste="pasteImages" @submit.prevent="saveNote" @keydown.ctrl.enter.prevent="saveNote"><div class="form-date">{{ editing?.date || selectedDate }}</div><div class="segmented note-kind-selector"><button v-for="item in (['progress', 'note', 'blocker'] as const)" :key="item" type="button" :class="{ active: kind === item }" @click="kind = item">{{ kindNames[item] }}</button></div><label class="field-label" for="note-content">记录内容</label><textarea id="note-content" v-model="content" class="note-editor" rows="7" maxlength="32000" autocomplete="off" placeholder="写下工作进展、备忘或问题，可粘贴图片…" />
      <div class="image-attach-toolbar"><button class="text-button" type="button" :disabled="busy || preparingImages || attachedImages.length >= maxNoteImages" title="最多 9 张，单张不超过 20 MB" @click="imageInput?.click()"><LoaderCircle v-if="preparingImages" class="spin" :size="16" /><ImagePlus v-else :size="16" />{{ preparingImages ? '处理图片…' : '上传图片' }}</button><span v-if="attachedImages.length" class="subtle">{{ attachedImages.length }} / {{ maxNoteImages }}</span><input ref="imageInput" type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/bmp,image/avif" multiple hidden aria-label="上传记录图片" @change="uploadImages" /></div>
      <NoteImages v-if="attachedImages.length" :images="attachedImages" :urls="imageUrls" editable :disabled="busy || preparingImages" @remove="removeImage" /><label class="field-label" for="note-tag">标签 <span>选填</span></label><input id="note-tag" v-model="tag" placeholder="添加标签" maxlength="40" autocomplete="off" /><footer class="modal-footer"><span class="subtle">Ctrl + Enter 保存</span><button class="button primary" type="submit" :disabled="busy || preparingImages || (!content.trim() && !attachedImages.length)"><LoaderCircle v-if="busy" class="spin" :size="16" />保存记录</button></footer></form></ModalShell>
  <ModalShell v-if="editGoal" :title="creatingGoal ? '新增待办' : '编辑待办'" @close="editGoal = null"><form class="modal-body" @submit.prevent="saveGoalEdit"><label class="field-label" for="goal-title">待办内容</label><input id="goal-title" v-model="editGoalTitle" maxlength="300" required autocomplete="off" placeholder="要做什么" autofocus /><label class="field-label" for="goal-project">项目 <span>选填，可直接输入新项目</span></label><input id="goal-project" v-model="editGoalProject" list="goal-projects" maxlength="40" autocomplete="off" placeholder="例如：客户 A、季度复盘" /><div class="field-label">优先级 <span>选填</span></div><div class="segmented goal-priority-selector"><button v-for="item in ([['', '未设置'], ['high', '高'], ['normal', '中'], ['low', '低']] as const)" :key="item[0]" type="button" :class="{ active: editGoalPriority === item[0] }" :aria-pressed="editGoalPriority === item[0]" @click="editGoalPriority = item[0]">{{ item[1] }}</button></div><div class="goal-time-fields"><div><label class="field-label" for="goal-estimate">预估耗时 <span>选填</span></label><input id="goal-estimate" v-model="editGoalEstimate" inputmode="text" autocomplete="off" placeholder="90、1.5h 或 1h30m" /></div><div><label class="field-label" for="goal-actual">实际耗时 <span>选填</span></label><input id="goal-actual" v-model="editGoalActual" inputmode="text" autocomplete="off" placeholder="分钟数，如 45" /></div></div><label class="field-label" for="goal-edit-risk">风险／原因 <span>选填，会写入报告</span></label><input id="goal-edit-risk" v-model="editGoalRisk" maxlength="300" autocomplete="off" placeholder="例如：等待外部评审，本日无法完成" /><label class="checkbox-label goal-adhoc-field"><input v-model="editGoalAdHoc" type="checkbox" />插活（中途临时安排，不在原计划内）</label><footer class="modal-footer"><button class="button secondary" type="button" @click="editGoal = null">取消</button><button class="button primary" type="submit" :disabled="busy || !editGoalTitle.trim()">{{ creatingGoal ? '添加待办' : '保存待办' }}</button></footer></form></ModalShell>
  <ModalShell v-if="carryTarget" title="顺延待办" @close="carryTarget = null"><form class="modal-body" @submit.prevent="confirmCarry"><div class="form-date">顺延到 {{ shortDate(carryTarget.toDate) }}</div><p class="confirm-copy">{{ carryTarget.goal.title }}</p><label class="field-label" for="goal-carry-risk">风险／原因 <span>选填，会写入报告</span></label><input id="goal-carry-risk" v-model="carryRisk" maxlength="300" autocomplete="off" placeholder="例如：等待外部评审，本日无法完成" /><p class="field-help">顺延后该待办移动到所选日期，首次计划日期与顺延次数保留。填写的原因会进入报告的风险与未完成原因章节。</p><footer class="modal-footer"><button class="button secondary" type="button" @click="carryTarget = null">取消</button><button class="button primary" type="submit" :disabled="busy">确认顺延</button></footer></form></ModalShell>
  <ModalShell v-if="deletion" title="删除记录" @close="deletion = null"><div class="modal-body"><p class="confirm-copy">删除后无法恢复，已生成的报告保留原有内容。</p><footer class="modal-footer"><button class="button secondary" @click="deletion = null">取消</button><button class="button danger" :disabled="busy" @click="confirmDelete">删除</button></footer></div></ModalShell>
</template>
