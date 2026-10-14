import { describe, expect, it } from 'vitest'
import { backlog, carryOver, delta, goalProject, goalsCsv, heatmap, parseMinutes, projectNames, projectShares, rangeFor, shiftRange, streak, summarize, toggled, trend } from './analytics'
import { localReport, statisticsFor, type Goal, type Note, type ReportRequest } from './domain'

const stamp = '2026-10-09T08:00:00.000Z'
function goal(date: string, extra: Partial<Goal> = {}): Goal {
  return { id: `${date}-${Math.random()}`, date, title: '事项', completed: false, createdAt: stamp, updatedAt: stamp, ...extra }
}
function note(date: string): Note {
  return { id: date + Math.random(), date, content: '内容', tag: '', kind: 'progress', createdAt: stamp, updatedAt: stamp, images: [] }
}

describe('待办项目与耗时', () => {
  it('优先使用项目字段，其次兼容标题写法，否则归入未分类', () => {
    expect(goalProject(goal('2026-10-09', { project: ' 客户 A ', title: '【别的】跟进' }))).toBe('客户 A')
    expect(goalProject(goal('2026-10-09', { title: '【官网】改版' }))).toBe('官网')
    expect(goalProject(goal('2026-10-09', { title: '周报：整理' }))).toBe('周报')
    expect(goalProject(goal('2026-10-09', { title: '打电话' }))).toBe('未分类')
  })
  it('项目名称按最近使用排序并去重，不预设固定项目', () => {
    const names = projectNames([goal('2026-10-01', { project: 'A', updatedAt: '2026-10-01T00:00:00Z' }), goal('2026-10-02', { project: 'B', updatedAt: '2026-10-05T00:00:00Z' }), goal('2026-10-03', { project: 'A', updatedAt: '2026-10-03T00:00:00Z' }), goal('2026-10-04')])
    expect(names).toEqual(['B', 'A'])
  })
  it('解析耗时输入并拒绝非法值', () => {
    expect(parseMinutes('')).toBeUndefined()
    expect(parseMinutes('90')).toBe(90)
    expect(parseMinutes('1.5h')).toBe(90)
    expect(parseMinutes('1h30m')).toBe(90)
    expect(parseMinutes('2小时')).toBe(120)
    expect(parseMinutes('45分钟')).toBe(45)
    expect(() => parseMinutes('abc')).toThrow()
    expect(() => parseMinutes('-5')).toThrow()
    expect(() => parseMinutes('100001')).toThrow()
  })
  it('统计和预估准确度只在两项都有记录时计算', () => {
    const summary = summarize([goal('2026-10-09', { completed: true, estimateMin: 60, actualMin: 90 }), goal('2026-10-09', { completed: true, actualMin: 30 }), goal('2026-10-09', { estimateMin: 20 })])
    expect(summary).toMatchObject({ total: 3, completed: 2, rate: 67, actualMin: 120, estimateMin: 80 })
    expect(summary.accuracy).toEqual({ count: 1, ratio: 1.5 })
    expect(summarize([goal('2026-10-09')]).accuracy).toBeNull()
  })
  it('领域统计保留耗时，日报正文仅包含项目和事项', () => {
    const goals = [goal('2026-10-09', { title: '写方案', project: '官网', completed: true, actualMin: 90 }), goal('2026-10-09', { title: '开会' })]
    const stats = statisticsFor([], goals)
    expect(stats).toMatchObject({ actualMinutes: 90, estimateMinutes: 0 })
    const request = { type: 'daily', style: 'concise', anchorDate: '2026-10-09', startDate: '', endDate: '', templateId: 'x', includeNotes: false, includeGoals: true } as unknown as ReportRequest
    const report = localReport({ notes: [], goals }, request)
    expect(report.markdown).toContain('官网')
    expect(report.markdown).not.toContain('耗时')
    expect(report.sources.goals[0]?.actualMin).toBe(90)
    const none = localReport({ notes: [], goals: [goals[1]] }, request)
    expect(JSON.stringify(none)).not.toContain('实际耗时')
  })
})

describe('完成时间与顺延', () => {
  it('勾选写入完成时间，取消勾选清除', () => {
    const done = toggled(goal('2026-10-09'), '2026-10-10T01:00:00.000Z')
    expect(done).toMatchObject({ completed: true, completedAt: '2026-10-10T01:00:00.000Z' })
    const undone = toggled(done)
    expect(undone.completed).toBe(false)
    expect('completedAt' in undone).toBe(false)
  })
  it('顺延只移动更早的未完成待办，保留首次计划日期并累计次数', () => {
    const moved = carryOver([goal('2026-10-08'), goal('2026-10-07', { originDate: '2026-10-01', carryCount: 2 }), goal('2026-10-08', { completed: true }), goal('2026-10-10'), goal('2026-10-11')], '2026-10-10', { now: stamp })
    expect(moved).toHaveLength(2)
    expect(moved.map(item => item.date)).toEqual(['2026-10-10', '2026-10-10'])
    expect(moved[0]).toMatchObject({ originDate: '2026-10-08', carryCount: 1, updatedAt: stamp })
    expect(moved[1]).toMatchObject({ originDate: '2026-10-01', carryCount: 3 })
  })
  it('顺延时可记录风险或原因，并沿用原有说明', () => {
    const moved = carryOver([goal('2026-10-08'), goal('2026-10-08', { risk: '依赖外部评审' })], '2026-10-09', { risk: '等待测试环境', now: stamp })
    expect(moved.map(item => item.risk)).toEqual(['等待测试环境', '等待测试环境'])
    const kept = carryOver([goal('2026-10-08', { risk: '依赖外部评审' })], '2026-10-09', { now: stamp })
    expect(kept[0].risk).toBe('依赖外部评审')
    const blank = carryOver([goal('2026-10-08', { risk: '旧原因' })], '2026-10-09', { risk: '   ', now: stamp })
    expect(blank[0].risk).toBe('旧原因')
  })
  it('积压按拖延时间排序，从首次计划日期开始计算', () => {
    const items = backlog([goal('2026-10-09'), goal('2026-10-09', { originDate: '2026-09-30' }), goal('2026-10-10'), goal('2026-10-01', { completed: true })], '2026-10-10')
    expect(items.map(item => item.ageDays)).toEqual([10, 1])
  })
})

describe('周期、趋势与热力图', () => {
  it('周、月、年范围及切换', () => {
    expect(rangeFor('week', '2026-10-10')).toEqual({ start: '2026-10-05', end: '2026-10-11' })
    expect(rangeFor('month', '2026-02-15')).toEqual({ start: '2026-02-01', end: '2026-02-28' })
    expect(rangeFor('year', '2026-10-10')).toEqual({ start: '2026-01-01', end: '2026-12-31' })
    expect(shiftRange('month', '2026-03-31', -1)).toBe('2026-02-28')
    expect(shiftRange('year', '2024-02-29', 1)).toBe('2025-02-28')
    expect(shiftRange('week', '2026-10-10', -1)).toBe('2026-10-03')
  })
  it('趋势按完成当天统计完成数，旧数据回退到计划日期', () => {
    const range = rangeFor('week', '2026-10-10')
    const buckets = trend([goal('2026-10-05', { completed: true, completedAt: new Date(2026, 9, 7, 12).toISOString(), actualMin: 30 }), goal('2026-10-06', { completed: true }), goal('2026-10-06')], range, 'week')
    expect(buckets).toHaveLength(7)
    expect(buckets[0]).toMatchObject({ planned: 1, completed: 0, actualMin: 30 })
    expect(buckets[2].completed).toBe(1)
    expect(buckets[1]).toMatchObject({ planned: 2, completed: 1 })
    expect(trend([], rangeFor('year', '2026-01-01'), 'year')).toHaveLength(12)
  })
  it('项目占比有耗时时按耗时排序', () => {
    const shares = projectShares([goal('2026-10-09', { project: 'A', actualMin: 10 }), goal('2026-10-09', { project: 'B', actualMin: 50, completed: true }), goal('2026-10-09', { project: 'B' }), goal('2026-10-09')])
    expect(shares.map(item => item.project)).toEqual(['B', 'A', '未分类'])
    expect(shares[0]).toMatchObject({ total: 2, completed: 1, actualMin: 50 })
  })
  it('热力图按最大值分档，连续记录从昨天往前算', () => {
    const days = heatmap([goal('2026-10-05'), goal('2026-10-05'), goal('2026-10-06')], [note('2026-10-05'), note('2026-10-05')], rangeFor('week', '2026-10-05'))
    expect(days.map(day => day.level)).toEqual([4, 1, 0, 0, 0, 0, 0])
    const goals = [goal('2026-10-08'), goal('2026-10-09'), goal('2026-10-06')]
    expect(streak(goals, [], '2026-10-10')).toBe(2)
    expect(streak(goals, [], '2026-10-09')).toBe(2)
    expect(streak([], [], '2026-10-10')).toBe(0)
  })
  it('环比变化不会出现除以零', () => {
    expect(delta(5, 0, ' 项')).toMatchObject({ direction: 'up', text: '较上期增加 5 项' })
    expect(delta(3, 3).text).toBe('与上期持平')
  })
})

describe('CSV 导出', () => {
  it('带 BOM、转义引号和换行，并阻止公式注入', () => {
    const csv = goalsCsv([goal('2026-10-09', { title: '=cmd|x', project: 'A,"B"', actualMin: 20 }), goal('2026-10-08', { title: '第一行\n第二行' })])
    expect(csv.startsWith('\uFEFF计划日期')).toBe(true)
    const lines = csv.split('\r\n')
    expect(lines[1]).toContain('"第一行\n第二行"')
    expect(csv).toContain("'=cmd|x")
    expect(csv).toContain('"A,""B"""')
  })
})
