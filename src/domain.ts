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
}

export interface Goal {
  id: string
  date: string
  title: string
  completed: boolean
  createdAt: string
  updatedAt: string
}

export interface Statistics {
  days: number
  notes: number
  goals: number
  completed: number
  blockers: number
  completionRate: number
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
  sources: { notes: Note[]; goals: Goal[] }
}

export interface AiSettings {
  endpoint: string
  model: string
  hasApiKey: boolean
}

export interface Workspace {
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
    notes: request.includeNotes ? workspace.notes.filter(note => note.date >= start && note.date <= end) : [],
    goals: request.includeGoals ? workspace.goals.filter(goal => goal.date >= start && goal.date <= end) : [],
  }
}

export function statisticsFor(notes: Note[], goals: Goal[]): Statistics {
  const completed = goals.filter(goal => goal.completed).length
  return {
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

export function localReport(workspace: Pick<Workspace, 'notes' | 'goals'>, request: ReportRequest): Report {
  const { start, end } = requestPeriod(request)
  const { notes, goals } = selectSources(workspace, request)
  if (!notes.length && !goals.length) throw new Error('所选范围内没有可用于生成报告的记录')
  const stats = statisticsFor(notes, goals)
  const title = `${shortDate(start)}${start !== end ? ` — ${shortDate(end)}` : ''}工作${typeNames[request.type]}`
  const lines = [`# ${title}`, '', periodLabel(start, end), '', '## 工作概况', '', `记录 ${stats.days} 天，共 ${stats.notes} 条备忘；${stats.goals} 项待办，已完成 ${stats.completed} 项${stats.goals ? `，完成率 ${stats.completionRate}%` : ''}。`, '']
  const done = goals.filter(goal => goal.completed)
  const pending = goals.filter(goal => !goal.completed)
  if (done.length) lines.push('## 已完成待办', '', ...done.map(goal => `- [x] ${goal.title}（${shortDate(goal.date)}）`), '')
  if (request.style === 'detailed') {
    const dates = [...new Set([...notes.map(note => note.date), ...goals.map(goal => goal.date)])].sort()
    lines.push('## 每日工作记录', '')
    dates.forEach(date => {
      lines.push(`### ${shortDate(date)}`, '')
      notes.filter(note => note.date === date).forEach(note => lines.push(`- ${kindNames[note.kind]}${note.tag ? ` · ${note.tag}` : ''}：${noteSummary(note).replace(/\n/g, '\n  ')}`))
      goals.filter(goal => goal.date === date).forEach(goal => lines.push(`- [${goal.completed ? 'x' : ' '}] ${goal.title}`))
      lines.push('')
    })
  } else {
    const regular = notes.filter(note => note.kind !== 'blocker')
    if (regular.length) lines.push(request.style === 'review' ? '## 过程与收获' : '## 工作进展', '', ...regular.map(note => `- ${note.tag ? `【${note.tag}】` : ''}${noteSummary(note).replace(/\n/g, '\n  ')}（${shortDate(note.date)}）`), '')
  }
  const blockers = notes.filter(note => note.kind === 'blocker')
  if (blockers.length) lines.push('## 问题与阻碍', '', ...blockers.map(note => `- ${noteSummary(note).replace(/\n/g, '\n  ')}（${shortDate(note.date)}）`), '')
  if (pending.length) lines.push(request.style === 'review' ? '## 待继续推进' : '## 未完成待办', '', ...pending.map(goal => `- [ ] ${goal.title}（${shortDate(goal.date)}）`), '')
  if (request.style === 'review') lines.push('## 复盘思考', '', '- 哪些做法值得继续？', '- 哪些问题需要调整？', '- 下一阶段优先推进什么？', '')
  return { id: crypto.randomUUID(), title, type: request.type, style: request.style, mode: 'local', startDate: start, endDate: end, markdown: lines.join('\n'), statistics: stats, createdAt: new Date().toISOString(), model: '', sources: { notes, goals } }
}
