import comparisonCases from '../tests/fixtures/plan-completion-comparison.json'
import { describe, expect, it } from 'vitest'
import { completionComparison, planCompletion, withPlanCompletion, withPlanStatus, carryPlans, fullWeeks, planComparisons, planTable, validatePlanWeek, type Plan, type PlanWeek } from './plans'
import { defaultReportTemplates, localReport, resolveReportTemplates, type ReportRequest, type Goal } from './domain'

const plan: Plan = { id: '00000000-0000-4000-8000-000000000001', archived: false, createdAt: '2026-10-05T09:00:00+08:00' }
const row: PlanWeek = { planId: plan.id, weekStart: '2026-10-05', title: '接口联调', project: '平台', status: 'blocked', progress: '完成字段映射', blocker: '等待测试环境', goalIds: [], carried: false, updatedAt: plan.createdAt }
const request: ReportRequest = { type: 'weekly', anchorDate: '2026-10-12', mode: 'local', style: 'brief', includeNotes: false, includeGoals: true, instruction: '' }

describe('weekly plans', () => {
  it('carries missed weeks exactly once without duplicating progress or losing the blocker', () => {
    const data = { plans: [plan], planWeeks: [row] }
    const rows = carryPlans(data, '2026-10-26')
    expect(rows.map(r => r.weekStart)).toEqual(['2026-10-05', '2026-10-12', '2026-10-19', '2026-10-26'])
    expect(rows.slice(1).every(r => r.progress === '' && r.carried && r.blocker === row.blocker)).toBe(true)
    expect(carryPlans({ ...data, planWeeks: rows }, '2026-10-26')).toEqual(rows)
    expect(row.progress).toBe('完成字段映射')
  })
  it('stops at completion and archive; preserves saved later weeks after historical changes', () => {
    expect(carryPlans({ plans: [plan], planWeeks: [{ ...row, status: 'completed', blocker: '' }] }, '2026-10-26')).toHaveLength(1)
    expect(carryPlans({ plans: [{ ...plan, archived: true }], planWeeks: [row] }, '2026-10-26')).toHaveLength(1)
    const later = { ...row, weekStart: '2026-10-19', progress: '重新开始', status: 'active' as const, blocker: '' }
    const rows = carryPlans({ plans: [plan], planWeeks: [{ ...row, status: 'completed', blocker: '' }, later] }, '2026-10-26')
    expect(rows.map(r => r.weekStart)).toEqual(['2026-10-05', '2026-10-19', '2026-10-26'])
    expect(rows[1]).toEqual(later)
  })
  it('crosses years and includes only fully contained calendar weeks', () => {
    expect(carryPlans({ plans: [plan], planWeeks: [{ ...row, weekStart: '2025-12-29' }] }, '2026-01-12').map(r => r.weekStart)).toEqual(['2025-12-29', '2026-01-05', '2026-01-12'])
    expect(fullWeeks('2026-10-06', '2026-10-25')).toEqual(['2026-10-12', '2026-10-19'])
    expect(fullWeeks('2026-10-13', '2026-10-18')).toEqual([])
  })
  it('requires blocker and rejects invalid links, dates and incomplete states', () => {
    expect(() => validatePlanWeek(row)).not.toThrow()
    expect(() => validatePlanWeek({ ...row, blocker: ' ' })).toThrow('阻塞原因')
    expect(() => validatePlanWeek({ ...row, status: 'active' })).toThrow('清空阻塞原因')
    expect(() => validatePlanWeek({ ...row, weekStart: '2026-10-06' })).toThrow('周一')
    expect(() => validatePlanWeek({ ...row, goalIds: ['a', 'a'] })).toThrow('关联待办')
  })
  it('generates plan-only reports and freezes both weeks and linked goals', () => {
    const goal: Goal = { id: 'task', date: '2026-10-05', title: '字段核对', completed: true, createdAt: plan.createdAt, updatedAt: plan.createdAt }
    const first = { ...row, goalIds: ['task'] }
    const data = { notes: [], goals: [goal], plans: [plan], planWeeks: carryPlans({ plans: [plan], planWeeks: [first] }, '2026-10-12') }
    const report = localReport(data, request)
    expect(report.statistics.goals).toBe(0)
    expect(report.sources.goals).toEqual([])
    expect(report.sources.planComparisons?.[0]?.goals).toEqual([goal])
    expect(report.markdown).toContain('尚未更新（自动延续）')
    expect(report.markdown).toContain('完成字段映射')
    data.planWeeks[0]!.progress = '改写'
    goal.title = '删除前改名'
    expect(report.sources.planComparisons?.[0]?.previous?.progress).toBe('完成字段映射')
    expect(report.sources.planComparisons?.[0]?.goals[0]?.title).toBe('字段核对')
  })
  it('keeps plans out of daily/monthly and appends comparison for custom templates', () => {
    const data = { notes: [], goals: [], plans: [plan], planWeeks: [row], settings: { endpoint: '', model: '', hasApiKey: false, reportTemplates: { ...defaultReportTemplates, weekly: '# 我的周报' } } }
    const req = { ...request, anchorDate: row.weekStart }
    expect(localReport(data, req).markdown).toContain('# 我的周报\n\n## 周计划进展对照')
    expect(() => localReport(data, { ...req, type: 'daily' })).toThrow()
    expect(() => localReport(data, { ...req, type: 'monthly' })).toThrow()
    expect(() => localReport({ notes: [], goals: [] }, req)).toThrow()
    expect(resolveReportTemplates({ weekly: '# 我的周报' }).weekly).toBe('# 我的周报')
    expect(resolveReportTemplates({ weekly: defaultReportTemplates.weekly.replace('## 周计划进展对照\n{{周计划进展对照}}\n\n', '') }).weekly).toBe(defaultReportTemplates.weekly)
  })
  it('escapes cell separators and HTML without losing task text', () => {
    const rows = planComparisons({ plans: [plan], planWeeks: [{ ...row, title: '<img>|事项', progress: '一\n二' }], goals: [] }, { ...request, anchorDate: row.weekStart })
    expect(planTable(rows)).toContain('＜img＞｜事项')
    expect(planTable(rows)).toContain('一；二')
  })
})


describe('plan completion', () => {
  it('reads old records and keeps slider and status consistent', () => {
    expect(planCompletion(row)).toBe(0)
    expect(planCompletion({ ...row, status: 'completed' })).toBe(100)
    const partial = withPlanCompletion(row, 65)
    expect(partial).toMatchObject({ completionPercent: 65, status: 'blocked', blocker: row.blocker })
    const completed = withPlanCompletion(partial, 100)
    expect(completed).toMatchObject({ completionPercent: 100, status: 'completed', blocker: '' })
    expect(withPlanCompletion(completed, 85)).toMatchObject({ completionPercent: 85, status: 'active', blocker: '' })
    const pending = withPlanStatus(completed, 'pending')
    expect(pending.completionPercent).toBe(0)
    expect(withPlanCompletion(pending, 1).status).toBe('active')
    expect(withPlanStatus(partial, 'completed')).toMatchObject({ completionPercent: 100, blocker: '' })
    expect(withPlanStatus(completed, 'active').completionPercent).toBe(99)
    expect(row.blocker).toBe('等待测试环境')
  })
  it('rejects invalid percentages and contradictory states', () => {
    for (const completionPercent of [-1, 101, 1.5, NaN, Infinity]) {
      expect(() => validatePlanWeek({ ...row, completionPercent })).toThrow('完成度')
    }
    expect(() => validatePlanWeek({ ...row, completionPercent: 100 })).toThrow('状态不一致')
    expect(() => validatePlanWeek({ ...row, completionPercent: 50, status: 'completed', blocker: '' })).toThrow('状态不一致')
    expect(() => validatePlanWeek(withPlanCompletion(row, 100))).not.toThrow()
  })
  it('carries percentages, stops completed plans and freezes report snapshots', () => {
    const partial = withPlanCompletion(row, 65)
    const data = { plans: [plan], planWeeks: carryPlans({ plans: [plan], planWeeks: [partial] }, '2026-10-12'), goals: [], notes: [] }
    expect(data.planWeeks.map(r => r.completionPercent)).toEqual([65, 65])
    const report = localReport(data, request)
    data.planWeeks[1] = withPlanCompletion(data.planWeeks[1]!, 100)
    expect(carryPlans(data, '2026-10-26')).toHaveLength(2)
    expect(report.sources.planComparisons?.[0]?.current.completionPercent).toBe(65)
    expect(report.sources.planComparisons?.[0]?.previous?.completionPercent).toBe(65)
  })
})


describe('report completion comparison', () => {
  for (const sample of comparisonCases) {
    it(sample.name, () => {
      const current = { ...row, ...sample.current, weekStart: '2026-10-12' } as PlanWeek
      const previous = sample.previous ? { ...row, ...sample.previous } as PlanWeek : undefined
      expect(completionComparison(current, previous)).toBe(sample.expected)
      const markdown = planTable([{ plan, current, previous, goals: [] }])
      expect(markdown).toContain('| 完成度（上周 → 本周） |')
      expect(markdown).toContain(`| ${sample.expected} |`)
    })
  }
})
