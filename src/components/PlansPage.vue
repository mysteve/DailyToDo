<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { Archive, ChevronLeft, ChevronRight, Plus } from '@lucide/vue'
import { api } from '../api'
import { addDays, dateKey, periodLabel } from '../domain'
import { planCompletion, planWeekId, withPlanCompletion, withPlanStatus, planStatusNames, validatePlanWeek, weekKey, type PlanWeek, type Plan, type PlanStatus } from '../plans'
import { busy, mutate, notify, refresh, workspace } from '../store'
import ModalShell from './ModalShell.vue'
import PlanCompletionSlider from './PlanCompletionSlider.vue'

const week = ref(weekKey(dateKey(new Date())))
const archived = ref(false)
const editor = ref<PlanWeek | null>(null)
const editingPlan = ref<Plan | null>(null)
const error = ref('')
const search = ref('')
const loading = ref(false)
const loadError = ref('')
const completionDrafts = ref<Record<string, number>>({})
function shownRow(row: PlanWeek) {
  const draft = completionDrafts.value[planWeekId(row)]
  return draft === undefined ? row : withPlanCompletion(row, draft)
}
async function saveCompletion(plan: Plan, row: PlanWeek, value: number) {
  const updated = { ...withPlanCompletion(row, value), carried: false, updatedAt: new Date().toISOString() }
  await mutate(() => api.savePlan(plan, updated), '完成度已保存')
  delete completionDrafts.value[planWeekId(row)]
}
const rows = computed(() => {
  const plans = (workspace.value.plans || []).filter(p => p.archived === archived.value)
  return plans.flatMap(plan => {
    const entries = (workspace.value.planWeeks || []).filter(row => row.planId === plan.id)
    const current = archived.value ? entries.sort((a, b) => b.weekStart.localeCompare(a.weekStart))[0] : entries.find(row => row.weekStart === week.value)
    if (!current) return []
    return [{ plan, current, previous: entries.find(row => row.weekStart === addDays(current.weekStart, -7)) }]
  }).sort((a, b) => a.plan.createdAt.localeCompare(b.plan.createdAt) || a.plan.id.localeCompare(b.plan.id))
})
const linked = computed(() => workspace.value.goals.filter(g => editor.value?.goalIds.includes(g.id)))
const choices = computed(() => workspace.value.goals.filter(g => `${g.title} ${g.project || ''} ${g.date}`.toLowerCase().includes(search.value.toLowerCase())).sort((a, b) => b.date.localeCompare(a.date)).slice(0, 100))
let loadSequence = 0
async function loadWeek() {
  const sequence = ++loadSequence
  loading.value = true; loadError.value = ''
  try { await api.carryPlans(week.value); await refresh() }
  catch (err) { if (sequence === loadSequence) loadError.value = String(err) }
  finally { if (sequence === loadSequence) loading.value = false }
}
watch(week, loadWeek, { immediate: true })
function selectWeek(event: Event) {
  const value = (event.target as HTMLInputElement).value
  if (value) { try { week.value = weekKey(value) } catch (err) { notify(String(err)) } }
}
function open(row?: PlanWeek) {
  error.value = ''; search.value = ''
  const now = new Date().toISOString()
  editingPlan.value = row ? { ...workspace.value.plans!.find(p => p.id === row.planId)! } : { id: crypto.randomUUID(), archived: false, createdAt: now }
  editor.value = row ? JSON.parse(JSON.stringify(row)) : { planId: editingPlan.value.id, weekStart: week.value, title: '', project: '', status: 'pending', completionPercent: 0, progress: '', blocker: '', goalIds: [], carried: false, updatedAt: now }
}
async function save() {
  if (!editor.value || !editingPlan.value) return
  const row = { ...editor.value, title: editor.value.title.trim(), project: editor.value.project.trim(), progress: editor.value.progress.trim(), blocker: editor.value.status === 'blocked' ? editor.value.blocker.trim() : '', carried: false, updatedAt: new Date().toISOString() }
  try { validatePlanWeek(row) } catch (err) { error.value = String(err); return }
  if (await mutate(() => api.savePlan(editingPlan.value!, row), '每周计划已保存')) editor.value = null
}
async function archive(plan: Plan) {
  await mutate(() => api.archivePlan(plan.id, !plan.archived), plan.archived ? '计划已恢复' : '计划已归档')
  if (plan.archived) await loadWeek()
}
</script>

<template>
  <div class="page-heading plans-heading"><div><h1>每周计划</h1><p class="subtle">{{ periodLabel(week, addDays(week, 6)) }}</p></div><div class="plan-actions"><button class="button primary" :disabled="busy || loading || !!loadError" @click="open()"><Plus :size="16" />新增计划</button></div></div>
  <div class="plan-toolbar"><div class="plan-week-control"><button class="icon-button" aria-label="上一周" :disabled="loading" @click="week = addDays(week, -7)"><ChevronLeft :size="18" /></button><input aria-label="选择计划周" type="date" :value="week" min="1900-01-01" max="9999-12-31" @change="selectWeek" /><button class="icon-button" aria-label="下一周" :disabled="loading" @click="week = addDays(week, 7)"><ChevronRight :size="18" /></button><button class="text-button" @click="week = weekKey(dateKey(new Date()))">本周</button></div><button class="text-button" :aria-pressed="archived" @click="archived = !archived"><Archive :size="15" />{{ archived ? '返回计划' : '已归档' }}</button></div>
  <div v-if="loadError" class="inline-error" role="alert">{{ loadError }} <button class="text-button" @click="loadWeek">重试</button></div>
  <div v-else-if="loading" class="empty-state">正在读取每周计划…</div>
  <div v-else-if="!rows.length" class="empty-state"><h3>{{ archived ? '暂无归档计划' : '本周暂无计划' }}</h3><button v-if="!archived" class="button secondary" @click="open()">新增计划</button></div>
  <div v-else class="plans-scroll"><div class="plan-grid plan-grid-heading"><span>计划</span><span>上周进展</span><span>本周推进</span><span>状态与阻塞</span><span /></div><article v-for="item in rows" :key="item.plan.id" class="plan-grid plan-row"><div><strong class="plan-title">{{ item.current.title }}</strong><span v-if="item.current.project" class="subtle plan-project">{{ item.current.project }}</span><span v-if="archived" class="subtle plan-project">{{ item.current.weekStart }}</span></div><div class="plan-copy"><span class="plan-mobile-label">上周进展</span><template v-if="item.previous"><span class="subtle">{{ planStatusNames[item.previous.status] }}</span><p>{{ item.previous.progress || (item.previous.carried ? '尚未更新' : '未记录') }}</p></template><span v-else class="subtle">本周新增</span></div><div class="plan-copy"><span class="plan-mobile-label">本周推进</span><p>{{ item.current.progress || (item.current.carried ? '尚未更新' : '未记录') }}</p><span v-if="item.current.carried" class="subtle">自动延续</span><PlanCompletionSlider :model-value="planCompletion(shownRow(item.current))" :label="`${item.current.title}完成度`" :disabled="busy || loading || archived" @update:model-value="completionDrafts[planWeekId(item.current)] = $event" @change="saveCompletion(item.plan, item.current, $event)" /></div><div class="plan-copy"><span class="plan-status" :class="shownRow(item.current).status">{{ planStatusNames[shownRow(item.current).status] }}</span><p v-if="shownRow(item.current).blocker" class="plan-blocker">{{ shownRow(item.current).blocker }}</p></div><div class="plan-row-actions"><button v-if="!archived" class="text-button" :disabled="busy" @click="open(item.current)">更新</button><button class="text-button" :disabled="busy" @click="archive(item.plan)">{{ archived ? '恢复' : '归档' }}</button></div></article></div>
  <ModalShell v-if="editor" title="每周计划" @close="editor = null"><form class="modal-body" @submit.prevent="save"><p class="subtle">{{ periodLabel(editor.weekStart, addDays(editor.weekStart, 6)) }}</p><label class="field-label" for="plan-title">计划名称</label><input id="plan-title" v-model="editor.title" required maxlength="300" autofocus /><label class="field-label" for="plan-project">项目 <span>选填</span></label><input id="plan-project" v-model="editor.project" maxlength="40" /><label class="field-label" for="plan-status">状态</label><select id="plan-status" :value="editor.status" @change="editor = withPlanStatus(editor, ($event.target as HTMLSelectElement).value as PlanStatus)"><option v-for="(name, status) in planStatusNames" :key="status" :value="status as PlanStatus">{{ name }}</option></select><PlanCompletionSlider :model-value="planCompletion(editor)" label="计划完成度" @update:model-value="editor = withPlanCompletion(editor, $event)" /><label class="field-label" for="plan-progress">本周推进</label><textarea id="plan-progress" v-model="editor.progress" rows="3" maxlength="2000" /><template v-if="editor.status === 'blocked'"><label class="field-label" for="plan-blocker">阻塞原因</label><textarea id="plan-blocker" v-model="editor.blocker" required rows="2" maxlength="300" /></template><details class="plan-links"><summary>关联待办 · {{ linked.filter(g => g.completed).length }} / {{ linked.length }} 已完成</summary><ul v-if="linked.length" class="plan-linked-list"><li v-for="goal in linked" :key="goal.id">{{ goal.completed ? '已完成' : '未完成' }} · {{ goal.title }}</li></ul><input v-model="search" aria-label="搜索可关联待办" placeholder="搜索待办、项目或日期" /><div class="plan-goal-choices"><label v-for="goal in choices" :key="goal.id" class="checkbox-label"><input v-model="editor.goalIds" type="checkbox" :value="goal.id" /><span>{{ goal.title }}<small>{{ goal.date }} · {{ goal.completed ? '已完成' : '未完成' }}</small></span></label><p v-if="!choices.length" class="subtle">暂无匹配待办</p><p v-if="choices.length === 100" class="subtle">显示前 100 项，可搜索缩小范围</p></div></details><p v-if="error" class="inline-error" role="alert">{{ error }}</p><footer class="modal-footer"><button class="button secondary" type="button" @click="editor = null">取消</button><button class="button primary" :disabled="busy" type="submit">保存</button></footer></form></ModalShell>
</template>
