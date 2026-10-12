import { addDays, dateKey, parseDate, periodFor, requestPeriod, type Goal, type ReportRequest } from './domain'

export type PlanStatus = 'pending' | 'active' | 'completed' | 'blocked'
export const planStatusNames: Record<PlanStatus, string> = { pending: '未开始', active: '进行中', completed: '已完成', blocked: '阻塞' }
export interface Plan { id: string; archived: boolean; createdAt: string }
export interface PlanWeek {
  planId: string; weekStart: string; title: string; project: string; status: PlanStatus
  progress: string; blocker: string; goalIds: string[]; carried: boolean; updatedAt: string
  completionPercent?: number
}
export interface PlanData { plans?: Plan[]; planWeeks?: PlanWeek[] }
export interface PlanComparison { plan: Plan; current: PlanWeek; previous?: PlanWeek; goals: Goal[] }
export const weekKey = (date: string) => periodFor('weekly', date).start
export const planWeekId = (row: PlanWeek) => `${row.planId}:${row.weekStart}`

export const planCompletion = (row: PlanWeek) => row.completionPercent ?? (row.status === 'completed' ? 100 : 0)

export function withPlanCompletion(row: PlanWeek, completionPercent: number): PlanWeek {
  const status = completionPercent === 100 ? 'completed' : row.status === 'completed' || (row.status === 'pending' && completionPercent > 0) ? 'active' : row.status
  return { ...row, completionPercent, status, blocker: status === 'blocked' ? row.blocker : '' }
}

export function withPlanStatus(row: PlanWeek, status: PlanStatus): PlanWeek {
  const completionPercent = status === 'completed' ? 100 : status === 'pending' ? 0 : Math.min(planCompletion(row), 99)
  return { ...row, status, completionPercent, blocker: status === 'blocked' ? row.blocker : '' }
}

export function validatePlan(plan: Plan) {
  if (!/^[\da-f]{8}-[\da-f]{4}-[\da-f]{4}-[\da-f]{4}-[\da-f]{12}$/i.test(plan.id) || typeof plan.archived !== 'boolean' || !Number.isFinite(Date.parse(plan.createdAt))) throw new Error('计划数据不正确')
}
export function validatePlanWeek(row: PlanWeek) {
  if (row.completionPercent !== undefined) {
    if (!Number.isInteger(row.completionPercent) || row.completionPercent < 0 || row.completionPercent > 100) throw new Error('完成度需为 0 至 100 的整数')
    if ((row.completionPercent === 100) !== (row.status === 'completed') || (row.status === 'pending' && row.completionPercent !== 0)) throw new Error('完成度与计划状态不一致')
  }
  if (!row.planId || weekKey(row.weekStart) !== row.weekStart || parseDate(row.weekStart).getFullYear() < 1900 || parseDate(row.weekStart).getFullYear() > 9999) throw new Error('计划日期需为有效的周一')
  if (typeof row.title !== 'string' || !row.title.trim() || [...row.title].length > 300 || typeof row.project !== 'string' || [...row.project].length > 40) throw new Error('计划名称需为 1 至 300 字，项目最多 40 字')
  if (!Object.hasOwn(planStatusNames, row.status) || typeof row.progress !== 'string' || [...row.progress].length > 2000 || typeof row.blocker !== 'string' || [...row.blocker].length > 300 || typeof row.carried !== 'boolean' || !Number.isFinite(Date.parse(row.updatedAt))) throw new Error('计划进展数据不正确')
  if (row.status === 'blocked' && !row.blocker.trim()) throw new Error('请填写阻塞原因')
  if (row.status !== 'blocked' && row.blocker.trim()) throw new Error('解除阻塞后请清空阻塞原因')
  if (!Array.isArray(row.goalIds) || row.goalIds.length > 500 || row.goalIds.some(id => typeof id !== 'string') || new Set(row.goalIds).size !== row.goalIds.length) throw new Error('关联待办不正确')
}

/** Only fills missing weeks; never changes a saved week, even after a historical correction. */
export function carryPlans(data: PlanData, through = dateKey(new Date())): PlanWeek[] {
  const target = weekKey(through)
  const result = structuredClone(data.planWeeks || [])
  for (const plan of data.plans || []) {
    if (plan.archived) continue
    const rows = result.filter(row => row.planId === plan.id && row.weekStart <= target).sort((a, b) => a.weekStart.localeCompare(b.weekStart))
    let previous = rows[0]
    if (!previous) continue
    const existing = new Map(rows.map(row => [row.weekStart, row]))
    for (let week = addDays(previous.weekStart, 7); week <= target; week = addDays(week, 7)) {
      const saved = existing.get(week)
      if (saved) { previous = saved; continue }
      if (previous.status === 'completed') continue
      const carried: PlanWeek = { ...previous, goalIds: [...previous.goalIds], weekStart: week, progress: '', carried: true, updatedAt: new Date().toISOString() }
      result.push(carried)
      previous = carried
    }
  }
  return result.sort((a, b) => planWeekId(a).localeCompare(planWeekId(b)))
}

export function fullWeeks(start: string, end: string): string[] {
  let week = weekKey(start)
  if (week < start) week = addDays(week, 7)
  const result: string[] = []
  for (; addDays(week, 6) <= end; week = addDays(week, 7)) result.push(week)
  return result
}

export function planComparisons(data: PlanData & { goals: Goal[] }, request: ReportRequest): PlanComparison[] {
  if (request.type !== 'weekly') return []
  const { start, end } = requestPeriod(request)
  const weeks = new Set(fullWeeks(start, end))
  return (data.planWeeks || []).filter(row => weeks.has(row.weekStart)).sort((a, b) => a.weekStart.localeCompare(b.weekStart) || a.planId.localeCompare(b.planId)).flatMap(current => {
    const plan = data.plans?.find(plan => plan.id === current.planId)
    if (!plan) return []
    const previous = data.planWeeks?.find(row => row.planId === current.planId && row.weekStart === addDays(current.weekStart, -7))
    const ids = new Set([...current.goalIds, ...(previous?.goalIds || [])])
    return [{ plan, current, previous, goals: data.goals.filter(goal => ids.has(goal.id)) }]
  })
}

const cell = (text: string) => text.replaceAll('|', '｜').replace(/[\r\n]+/g, '；').replaceAll('<', '＜').replaceAll('>', '＞')
// Missing historical percentages are unknown, not evidence of zero progress.
export function completionComparison(current: PlanWeek, previous?: PlanWeek): string {
  const known = (row: PlanWeek) => row.completionPercent ?? (row.status === 'completed' ? 100 : undefined)
  const before = previous ? known(previous) : undefined
  const after = known(current)
  const label = (value: number | undefined) => value === undefined ? '未记录' : `${value}%`
  let result = `${previous ? label(before) : '本周新增'} → ${label(after)}`
  const details: string[] = []
  if (before !== undefined && after !== undefined) {
    const change = after - before
    details.push(change === 0 ? '持平' : `${change > 0 ? '+' : ''}${change} 个百分点`)
  }
  if (current.carried) details.push('自动延续')
  if (details.length) result += `（${details.join('；')}）`
  return result
}

export function planTable(rows: PlanComparison[]): string {
  if (!rows.length) return ''
  return [...new Set(rows.map(row => row.current.weekStart))].sort().map(week => {
    const items = rows.filter(row => row.current.weekStart === week)
    const lines = items.map(({ current: c, previous: p }) => `| ${[c.project ? `${c.project}：${c.title}` : c.title, completionComparison(c, p), p ? `${planStatusNames[p.status]}；${p.progress || (p.carried ? '尚未更新' : '未记录')}` : '本周新增', c.progress || (c.carried ? '尚未更新（自动延续）' : '未记录'), planStatusNames[c.status], c.status === 'blocked' ? c.blocker : '—'].map(cell).join(' | ')} |`)
    return `### ${week} — ${addDays(week, 6)}\n\n计划 ${items.length} 项 · 已完成 ${items.filter(row => row.current.status === 'completed').length} 项 · 阻塞 ${items.filter(row => row.current.status === 'blocked').length} 项\n\n| 计划 | 完成度（上周 → 本周） | 上周状态与进展 | 本周推进 | 当前状态 | 阻塞原因 |\n| --- | --- | --- | --- | --- | --- |\n${lines.join('\n')}`
  }).join('\n\n')
}
