import { addDays, dateKey, formatMinutes, parseDate, periodFor, type DateRange, type Goal, type Note } from './domain'

export type InsightKind = 'week' | 'month' | 'year'
export const insightNames: Record<InsightKind, string> = { week: '周', month: '月', year: '年' }
export const uncategorized = '未分类'

/** 待办的项目：优先使用 project 字段，其次兼容标题中的「项目：事项」「【项目】事项」写法 */
export function goalProject(goal: Pick<Goal, 'project' | 'title'>): string {
  const explicit = goal.project?.trim()
  if (explicit) return explicit
  const title = goal.title.trim()
  const labelled = title.match(/^(?:【([^】]*)】|\[([^\]]*)\])(.*)$/s)
  const separated = labelled ? null : title.match(/^([^：:]*)[：:](.*)$/s)
  const project = (labelled?.[1] ?? labelled?.[2] ?? separated?.[1])?.trim()
  const content = (labelled?.[3] ?? separated?.[2])?.trim()
  return project && content && !project.includes('\n') && !content.includes('\n') ? project : uncategorized
}

/** 完成当天；旧数据没有 completedAt 时，按计划日期计 */
export function completedDate(goal: Goal): string {
  return goal.completedAt ? dateKey(new Date(goal.completedAt)) : goal.date
}

export function inRange<T extends { date: string }>(items: T[], range: DateRange): T[] {
  return items.filter(item => item.date >= range.start && item.date <= range.end)
}

export function rangeFor(kind: InsightKind, anchor: string): DateRange {
  if (kind === 'week') return periodFor('weekly', anchor)
  if (kind === 'month') return periodFor('monthly', anchor)
  const year = parseDate(anchor).getFullYear()
  return { start: `${year}-01-01`, end: `${year}-12-31` }
}

export function shiftRange(kind: InsightKind, anchor: string, amount: number): string {
  const date = parseDate(anchor)
  if (kind === 'week') return addDays(anchor, amount * 7)
  const day = date.getDate()
  date.setDate(1)
  if (kind === 'month') date.setMonth(date.getMonth() + amount)
  else date.setFullYear(date.getFullYear() + amount)
  date.setDate(Math.min(day, new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate()))
  return dateKey(date)
}

export interface PeriodSummary {
  total: number
  completed: number
  rate: number
  actualMin: number
  estimateMin: number
  recordedDays: number
  /** 同时记录了预估和实际耗时的已完成待办的偏差，实际 / 预估 */
  accuracy: { count: number; ratio: number } | null
}

export function summarize(goals: Goal[], notes: Note[] = []): PeriodSummary {
  const completed = goals.filter(goal => goal.completed).length
  const both = goals.filter(goal => goal.completed && goal.estimateMin && goal.actualMin !== undefined)
  const estimate = both.reduce((sum, goal) => sum + goal.estimateMin!, 0)
  return {
    total: goals.length,
    completed,
    rate: goals.length ? Math.round(completed / goals.length * 100) : 0,
    actualMin: goals.reduce((sum, goal) => sum + (goal.actualMin || 0), 0),
    estimateMin: goals.reduce((sum, goal) => sum + (goal.estimateMin || 0), 0),
    recordedDays: new Set([...goals.map(goal => goal.date), ...notes.map(note => note.date)]).size,
    accuracy: both.length && estimate ? { count: both.length, ratio: both.reduce((sum, goal) => sum + goal.actualMin!, 0) / estimate } : null,
  }
}

export interface Delta { value: number; text: string; direction: 'up' | 'down' | 'flat' }

/** 与上一周期比较；上一周期为 0 时不给百分比，避免出现无意义的无穷大 */
export function delta(current: number, previous: number, unit = ''): Delta {
  const diff = current - previous
  const direction = diff > 0 ? 'up' : diff < 0 ? 'down' : 'flat'
  if (diff === 0) return { value: 0, text: '与上期持平', direction }
  return { value: diff, text: `较上期${diff > 0 ? '增加' : '减少'} ${Math.abs(diff)}${unit}`, direction }
}

export interface Bucket { key: string; label: string; planned: number; completed: number; actualMin: number }

/** 周、月按天，年按月；「完成」按完成当天统计，「新增」按计划日期统计 */
export function trend(goals: Goal[], range: DateRange, kind: InsightKind): Bucket[] {
  const buckets = new Map<string, Bucket>()
  if (kind === 'year') {
    for (let month = 1; month <= 12; month++) {
      const key = `${range.start.slice(0, 4)}-${String(month).padStart(2, '0')}`
      buckets.set(key, { key, label: `${month}月`, planned: 0, completed: 0, actualMin: 0 })
    }
  } else {
    for (let date = range.start; date <= range.end; date = addDays(date, 1)) {
      buckets.set(date, { key: date, label: String(Number(date.slice(8))), planned: 0, completed: 0, actualMin: 0 })
    }
  }
  const keyOf = (date: string) => kind === 'year' ? date.slice(0, 7) : date
  for (const goal of goals) {
    const planned = buckets.get(keyOf(goal.date))
    if (planned) { planned.planned++; planned.actualMin += goal.actualMin || 0 }
    if (goal.completed) {
      const done = buckets.get(keyOf(completedDate(goal)))
      if (done) done.completed++
    }
  }
  return [...buckets.values()]
}

export interface ProjectShare { project: string; total: number; completed: number; actualMin: number; estimateMin: number }

/** 按项目汇总；有耗时记录时按耗时排序，否则按待办数 */
export function projectShares(goals: Goal[]): ProjectShare[] {
  const map = new Map<string, ProjectShare>()
  for (const goal of goals) {
    const project = goalProject(goal)
    const item = map.get(project) || { project, total: 0, completed: 0, actualMin: 0, estimateMin: 0 }
    item.total++
    if (goal.completed) item.completed++
    item.actualMin += goal.actualMin || 0
    item.estimateMin += goal.estimateMin || 0
    map.set(project, item)
  }
  return [...map.values()].sort((a, b) => b.actualMin - a.actualMin || b.total - a.total || a.project.localeCompare(b.project, 'zh-CN'))
}

export function projectNames(goals: Goal[]): string[] {
  const latest = new Map<string, string>()
  for (const goal of goals) {
    const project = goal.project?.trim()
    if (project && (latest.get(project) ?? '') < goal.updatedAt) latest.set(project, goal.updatedAt)
  }
  return [...latest].sort((a, b) => b[1].localeCompare(a[1])).map(([project]) => project)
}

export interface HeatDay { date: string; count: number; level: 0 | 1 | 2 | 3 | 4 }

/** 每天的留痕数量（待办 + 手记），用于热力图；level 按当期最大值分档 */
export function heatmap(goals: Goal[], notes: Note[], range: DateRange): HeatDay[] {
  const counts = new Map<string, number>()
  for (const item of [...goals, ...notes]) counts.set(item.date, (counts.get(item.date) || 0) + 1)
  const days: HeatDay[] = []
  for (let date = range.start; date <= range.end; date = addDays(date, 1)) days.push({ date, count: counts.get(date) || 0, level: 0 })
  const max = Math.max(0, ...days.map(day => day.count))
  for (const day of days) day.level = day.count === 0 ? 0 : (Math.min(4, Math.max(1, Math.ceil(day.count / max * 4))) as 1 | 2 | 3 | 4)
  return days
}

/** 截至 today 的连续留痕天数；今天还没记录时不打断，从昨天往前算 */
export function streak(goals: Goal[], notes: Note[], today: string): number {
  const dates = new Set([...goals.map(goal => goal.date), ...notes.map(note => note.date)])
  let date = dates.has(today) ? today : addDays(today, -1)
  let count = 0
  while (dates.has(date)) { count++; date = addDays(date, -1) }
  return count
}

export interface BacklogItem { goal: Goal; ageDays: number }

export function daysBetween(from: string, to: string): number {
  return Math.round((parseDate(to).getTime() - parseDate(from).getTime()) / 86400000)
}

/** 早于 today 且仍未完成的待办，按拖延最久排序；age 以首次计划日期计 */
export function backlog(goals: Goal[], today: string): BacklogItem[] {
  return goals
    .filter(goal => !goal.completed && goal.date < today)
    .map(goal => ({ goal, ageDays: daysBetween(goal.originDate || goal.date, today) }))
    .sort((a, b) => b.ageDays - a.ageDays || (b.goal.carryCount || 0) - (a.goal.carryCount || 0))
}

/** 把未完成待办顺延到指定日期，保留首次计划日期、已有风险说明并累计顺延次数；不会改动已完成或已在目标日期的待办 */
export function carryOver(goals: Goal[], toDate: string, options: { risk?: string; now?: string } = {}): Goal[] {
  const now = options.now || new Date().toISOString()
  const risk = options.risk?.trim()
  return goals
    .filter(goal => !goal.completed && goal.date < toDate)
    .map(goal => {
      const next: Goal = { ...goal, date: toDate, originDate: goal.originDate || goal.date, carryCount: (goal.carryCount || 0) + 1, updatedAt: now }
      if (risk) next.risk = risk
      return next
    })
}

/** 勾选或取消完成：写入或清除完成时间 */
export function toggled(goal: Goal, now = new Date().toISOString()): Goal {
  const next: Goal = { ...goal, completed: !goal.completed, updatedAt: now }
  if (next.completed) next.completedAt = now
  else delete next.completedAt
  return next
}

/** 解析耗时输入：支持「90」「1.5h」「1h30m」「1小时30分」，返回分钟；空字符串返回 undefined，非法输入抛错 */
export function parseMinutes(input: string): number | undefined {
  const text = input.trim().toLowerCase().replace(/\s+/g, '')
  if (!text) return undefined
  let total: number
  const plain = text.match(/^(\d+)(?:m|min|分|分钟)?$/)
  const hours = text.match(/^(\d+(?:\.\d+)?)(?:h|小时)$/)
  const mixed = text.match(/^(\d+)(?:h|小时)(\d+)(?:m|min|分|分钟)?$/)
  if (plain) total = Number(plain[1])
  else if (hours) total = Math.round(Number(hours[1]) * 60)
  else if (mixed) total = Number(mixed[1]) * 60 + Number(mixed[2])
  else throw new Error('耗时格式不正确，可填写 90、1.5h 或 1h30m')
  if (total > 100000) throw new Error('耗时不能超过 100000 分钟')
  return total
}

export function minutesInput(minutes: number | undefined): string {
  return minutes === undefined ? '' : String(minutes)
}

export function shortMinutes(minutes: number): string {
  return minutes ? formatMinutes(minutes) : '0'
}

function csvCell(value: string | number | undefined): string {
  let text = value === undefined ? '' : String(value)
  // 防止在 Excel 中被当作公式执行
  if (/^[=+\-@\t\r]/.test(text) && !/^-?\d+(\.\d+)?$/.test(text)) text = `'${text}`
  return /[",\r\n]/.test(text) ? `"${text.replace(/"/g, '""')}"` : text
}

const priorityText = { high: '高', normal: '中', low: '低' } as const

/** 导出待办明细为 CSV，带 BOM 以便 Excel 正确识别中文 */
export function goalsCsv(goals: Goal[]): string {
  const header = ['计划日期', '首次计划日期', '待办', '项目', '状态', '完成时间', '优先级', '预估耗时(分钟)', '实际耗时(分钟)', '顺延次数', '风险或原因', '插活']
  const rows = [...goals].sort((a, b) => a.date.localeCompare(b.date) || a.createdAt.localeCompare(b.createdAt)).map(goal => [
    goal.date, goal.originDate, goal.title, goalProject(goal) === uncategorized ? '' : goalProject(goal), goal.completed ? '已完成' : '未完成',
    goal.completedAt ? dateKey(new Date(goal.completedAt)) : '', goal.priority ? priorityText[goal.priority] : '', goal.estimateMin, goal.actualMin, goal.carryCount || 0,
    goal.risk, goal.adHoc ? '是' : '',
  ].map(csvCell).join(','))
  return '\uFEFF' + [header.join(','), ...rows].join('\r\n') + '\r\n'
}
