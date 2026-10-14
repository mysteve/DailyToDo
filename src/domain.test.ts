import { describe, expect, it } from 'vitest'
import { defaultReportTemplates, gitTokenPage, resolveReportTemplates, validateReportTemplates, localReport, periodFor, requestPeriod, selectSources, statisticsFor, type Goal, type Note, type ReportRequest } from './domain'

const request: ReportRequest = { type: 'weekly', anchorDate: '2026-10-09', style: 'brief', mode: 'local', includeNotes: true, includeGoals: true, instruction: '' }
const note = (date: string, kind: Note['kind'] = 'progress'): Note => ({ id: crypto.randomUUID(), date, kind, content: '完成页面设计', tag: '设计', createdAt: `${date}T09:00:00+08:00`, updatedAt: `${date}T09:00:00+08:00` })
const goal = (date: string, completed: boolean): Goal => ({ id: crypto.randomUUID(), date, title: completed ? '完成设计' : '待确认接口', completed, createdAt: `${date}T09:00:00+08:00`, updatedAt: `${date}T09:00:00+08:00` })

describe('按用户选择的周期整理报告', () => {
  it('日报明日计划包含截至当天的所有积压，排除往日完成项和未来待办', () => {
    const goals = [
      { ...goal('2026-09-30', false), title: '积压目标：完成接口联调' },
      { ...goal('2026-10-08', true), title: '昨日已完成' },
      { ...goal('2026-10-09', true), title: '今日交付' },
      { ...goal('2026-10-09', false), title: '当天未完成' },
      { ...goal('2026-10-10', false), title: '未来待办' },
    ]
    const data = { notes: [], goals }
    for (const style of ['brief', 'detailed', 'review'] as const) {
      const report = localReport(data, { ...request, type: 'daily', style })
      const [completed, planned] = report.markdown.split('【明日重点计划】')
      expect(completed).toContain('今日交付')
      expect(completed).not.toContain('积压目标')
      expect(planned?.trim()).toBe('1、积压目标：完成接口联调\n2、当天未完成')
      expect(report.markdown).not.toMatch(/昨日已完成|未来待办/)
      expect(report.sources.goals).toHaveLength(3)
      expect(report.statistics).toMatchObject({ goals: 3, completed: 1 })
    }
    const onlyBacklog = localReport({ notes: [], goals: [goals[0]!] }, { ...request, type: 'daily' })
    expect(onlyBacklog.markdown).toContain('【明日重点计划】\n1、积压目标：完成接口联调')
    expect(localReport(data, request).sources.goals).toHaveLength(4)
    expect(localReport(data, { ...request, type: 'monthly' }).sources.goals).toHaveLength(4)
  })

  it('日报自定义模板缺少计划字段时补充明日计划，已有字段时不重复', () => {
    const custom = '# 我的日报\n{{项目完成情况}}'
    const settings = { endpoint: '', model: '', hasApiKey: false, reportTemplates: { ...defaultReportTemplates, daily: custom } }
    const data = { notes: [], goals: [goal('2026-10-08', false)], settings }
    expect(localReport(data, { ...request, type: 'daily' }).markdown).toBe('# 我的日报\n暂无\n\n## 【明日重点计划】\n1、待确认接口')
    expect(settings.reportTemplates.daily).toBe(custom)
    for (const token of ['项目计划', '下阶段重点任务与交付', '未完成待办']) {
      settings.reportTemplates.daily = `# 日报\n## 明日计划\n{{ ${token} }}`
      expect(localReport(data, { ...request, type: 'daily' }).markdown).toBe('# 日报\n## 明日计划\n1、待确认接口')
    }
  })

  it('周从周一开始，跨年周与闰月计算准确', () => {
    expect(periodFor('weekly', '2027-01-01')).toEqual({ start: '2026-12-28', end: '2027-01-03' })
    expect(periodFor('weekly', '2026-10-11')).toEqual({ start: '2026-10-05', end: '2026-10-11' })
    expect(periodFor('monthly', '2024-02-15')).toEqual({ start: '2024-02-01', end: '2024-02-29' })
    expect(() => periodFor('daily', '2026-02-30')).toThrow()
  })
  it('仅包含日期范围内的所选来源', () => {
    const data = { notes: [note('2026-10-04'), note('2026-10-05'), note('2026-10-11'), note('2026-10-12')], goals: [goal('2026-10-09', true)] }
    expect(selectSources(data, request).notes).toHaveLength(0)
    expect(selectSources(data, { ...request, includeNotes: false }).notes).toHaveLength(0)
    expect(selectSources(data, { ...request, includeGoals: false }).goals).toHaveLength(1)
  })
  it('自选范围保留跨月、跨年边界，并包含起止日', () => {
    const data = { notes: [note('2026-09-29'), note('2026-09-30'), note('2026-10-02'), note('2026-10-03')], goals: [goal('2026-09-30', true), goal('2026-10-02', false), goal('2026-10-03', true)] }
    for (const type of ['weekly', 'monthly'] as const) {
      const custom = { ...request, type, startDate: '2026-09-30', endDate: '2026-10-02' }
      const report = localReport(data, custom)
      expect([report.startDate, report.endDate]).toEqual(['2026-09-30', '2026-10-02'])
      expect(report.sources.notes).toEqual([])
      expect(report.statistics).toMatchObject({ days: 2, notes: 0, goals: 2, completed: 1, completionRate: 50 })
    }
    expect(requestPeriod({ ...request, startDate: '2026-12-31', endDate: '2027-01-02' })).toEqual({ start: '2026-12-31', end: '2027-01-02' })
    expect(requestPeriod({ ...request, startDate: '2026-10-09', endDate: '2026-10-09' })).toEqual({ start: '2026-10-09', end: '2026-10-09' })
  })
  it('自选日期缺失、倒序、无效或超出支持年份时拒绝生成', () => {
    for (const dates of [
      { startDate: '2026-10-09' },
      { startDate: '', endDate: '2026-10-09' },
      { startDate: '2026-10-10', endDate: '2026-10-09' },
      { startDate: '2026-02-30', endDate: '2026-03-01' },
      { startDate: '1899-12-31', endDate: '1900-01-01' },
    ]) expect(() => localReport({ notes: [note('2026-10-09')], goals: [] }, { ...request, ...dates })).toThrow()
    expect(requestPeriod({ ...request, type: 'daily', startDate: '2026-10-01', endDate: '2026-10-31' })).toEqual({ start: '2026-10-09', end: '2026-10-09' })
  })
  it('记录天数按日期去重，目标完成率以所选目标计算', () => {
    const stats = statisticsFor([note('2026-10-09'), note('2026-10-09', 'blocker')], [goal('2026-10-09', true), goal('2026-10-08', false), goal('2026-10-08', false)])
    expect(stats).toEqual({ days: 2, notes: 2, goals: 3, completed: 1, blockers: 1, completionRate: 33, estimateMinutes: 0, actualMinutes: 0 })
    expect(statisticsFor([], []).completionRate).toBe(0)
  })
  it('保存真实来源，并区分已完成和未完成目标', () => {
    const data = { notes: [note('2026-10-09')], goals: [goal('2026-10-09', true), goal('2026-10-09', false)] }
    const report = localReport(data, request)
    const [completed, planned] = report.markdown.split('【下周重点计划】')
    expect(completed).toContain('①成果或进度：完成设计')
    expect(completed).not.toContain('待确认接口')
    expect(planned).toContain('①重点任务：待确认接口')
    expect(planned).not.toContain('完成设计')
    expect(report.sources.notes).toHaveLength(0)
    expect(report.mode).toBe('local')
  })
  it('空范围不保存报告，详细汇报按日展开', () => {
    expect(() => localReport({ notes: [], goals: [] }, request)).toThrow('没有可用于生成')
    const report = localReport({ notes: [note('2026-10-09')], goals: [goal('2026-10-09', true)] }, { ...request, style: 'detailed' })
    expect(report.markdown).toContain('### 10月9日')
  })
  it('工作记录和图片不能成为报告来源，统计仍保留留痕', () => {
    const imageNote = { ...note('2026-10-09'), content: '记录专有内容', images: [{ id: crypto.randomUUID(), name: '截图.png', mime: 'image/png', width: 1200, height: 800 }] }
    for (const type of ['daily', 'weekly', 'monthly'] as const) {
      for (const style of ['brief', 'detailed', 'review'] as const) {
        expect(() => localReport({ notes: [imageNote], goals: [] }, { ...request, type, style })).toThrow('待办事项')
        const report = localReport({ notes: [imageNote], goals: [goal('2026-10-09', true)] }, { ...request, type, style, includeGoals: false })
        expect(report.sources.notes).toEqual([])
        expect(report.statistics.notes).toBe(0)
        expect(report.markdown).not.toContain('记录专有内容')
        expect(report.markdown).not.toContain('截图.png')
      }
    }
    expect(statisticsFor([imageNote], []).notes).toBe(1)
  })
  it('按报告类型使用模板，待办中的变量不会被再次替换', () => {
    const templates = { daily: '# 日定制\n{{已完成待办}}', weekly: '# 周定制\n{{未完成待办}}', monthly: '# 月定制\n{{概况}}\n{{每日待办}}' }
    const data = { notes: [], goals: [ { ...goal('2026-10-09', true), title: '处理 {{未完成待办}}' }, goal('2026-10-09', false)], settings: { endpoint: '', model: '', hasApiKey: false, reportTemplates: templates } }
    for (const type of ['daily', 'weekly', 'monthly'] as const) {
      const report = localReport(data, { ...request, type })
      expect(report.markdown).toContain({ daily: '日定制', weekly: '周定制', monthly: '月定制' }[type])
    }
    expect(localReport(data, { ...request, type: 'daily' }).markdown).toContain('处理 {{未完成待办}}')
    expect(() => validateReportTemplates({ ...defaultReportTemplates, daily: '' })).toThrow('日报')
    expect(() => validateReportTemplates({ ...defaultReportTemplates, weekly: '{{项目名称}}' })).not.toThrow()
    expect(() => validateReportTemplates({ ...defaultReportTemplates, monthly: '字'.repeat(10001) })).toThrow('10000')
  })
  it('截图中的自定义模板可以保存，本地日期填充且其他字段明确待填写', () => {
    const templates = { ...defaultReportTemplates, weekly: '{{日期年-月-日}}完成任务进度和计划\n【完成情况】：\n1、{{项目名称}}\n①{{完成的事项1}}\n【明日计划】\n{{计划完成的事项1}}\n{{已完成待办}}' }
    const report = localReport({ notes: [note('2026-10-09')], goals: [goal('2026-10-09', true)], settings: { endpoint: '', model: '', hasApiKey: false, reportTemplates: templates } }, request)
    expect(report.markdown).toContain('2026-10-11完成任务进度和计划')
    expect(report.markdown).toContain('待填写：项目名称')
    expect(report.markdown).toContain('待填写：完成的事项1')
    expect(report.markdown).toContain('待填写：计划完成的事项1')
    expect(report.markdown).toContain('- [x] 完成设计')
    expect(report.markdown).not.toContain('undefined')
    expect(report.sources.notes).toEqual([])
  })
  it('上级汇报模板按周期突出成果和计划，缺失的价值、风险与时间不能编造', () => {
    const goals = [
      { ...goal('2026-10-09', true), title: '项目名称1：完成的内容或进度1' },
      { ...goal('2026-10-09', true), title: '【项目名称1】完成的内容或进度2' },
      { ...goal('2026-10-09', true), title: '[项目名称2]完成的内容或进度3' },
      { ...goal('2026-10-09', false), title: '项目名称1：计划内容1' },
      { ...goal('2026-10-09', false), title: '未填写项目的事项' },
    ]
    for (const [type, header, plan] of [
      ['weekly', '# 周报\n\n2026-10-05 — 2026-10-11', '下周重点计划'],
      ['monthly', '# 月报\n\n2026-10-01 — 2026-10-31', '下月重点计划'],
    ] as const) {
      const report = localReport({ notes: [], goals }, { ...request, type })
      expect(report.markdown.startsWith(`${header}\n`)).toBe(true)
      const planAt = report.markdown.indexOf(`## 【${plan}】`)
      const resultsAt = report.markdown.indexOf('①成果或进度：完成的内容或进度1')
      expect(resultsAt).toBeGreaterThan(0)
      expect(planAt).toBeGreaterThan(resultsAt)
      expect(report.markdown).toContain('1、项目名称1：\n①成果或进度：完成的内容或进度1\n②成果或进度：完成的内容或进度2\n③实际影响：待确认\n2、项目名称2：\n①成果或进度：完成的内容或进度3\n②实际影响：待确认')
      expect(report.markdown.slice(planAt)).toContain('1、项目名称1：\n①重点任务：计划内容1\n②预期交付及完成时间：待确认\n2、待办事项：\n①重点任务：未填写项目的事项')
      expect(report.markdown.slice(planAt)).not.toContain('完成的内容或进度')
      expect(report.markdown).toContain('风险、进度影响及需协调事项：待确认')
      expect(report.markdown).not.toContain('{{')
      expect(report.markdown).not.toContain('目标达成率')
      if (type === 'monthly') {
        expect(report.markdown).toContain('月度目标、验收标准及达成情况：待确认')
        expect(report.markdown).toContain('未完成原因：待确认')
      }
      expect(report.statistics).toMatchObject({ goals: 5, completed: 3, completionRate: 60 })
    }
    const emptyCompleted = localReport({ notes: [], goals: [goal('2026-10-09', false)] }, request)
    expect(emptyCompleted.markdown).toContain('【重点项目与节点进展】\n暂无')
    const emptyPlan = localReport({ notes: [], goals: [goal('2026-10-09', true)] }, request)
    expect(emptyPlan.markdown).toContain('【下周重点计划】\n暂无')
    const monthly = localReport({ notes: [], goals: [goal('2026-10-09', true)] }, { ...request, type: 'monthly' })
    expect(monthly.markdown).toContain('所选范围内无未完成待办。')
    expect(monthly.markdown).toContain('月度目标、验收标准及达成情况：待确认')
  })
  it('待办填写的风险或原因进入报告，未填写时仍标记待确认', () => {
    const goals = [
      { ...goal('2026-10-09', false), title: '接口联调', risk: '依赖外部联调环境，本日无法完成' },
      { ...goal('2026-10-09', false), title: '采购询价' },
    ]
    const monthly = { ...request, type: 'monthly' } as const
    const report = localReport({ notes: [], goals }, monthly)
    expect(report.markdown).toContain('风险、进度影响及需协调事项：\n①接口联调：依赖外部联调环境，本日无法完成')
    expect(report.markdown).toContain('其余 1 项未记录风险或问题：待确认')
    expect(report.markdown).toContain('未完成原因：\n①接口联调：依赖外部联调环境，本日无法完成')
    const plain = localReport({ notes: [], goals: [goal('2026-10-09', false)] }, monthly)
    expect(plain.markdown).toContain('风险、进度影响及需协调事项：待确认')
    expect(plain.markdown).toContain('未完成原因：待确认')
    const templates = { ...defaultReportTemplates, weekly: '{{未完成待办}}' }
    const settings = { endpoint: '', model: '', hasApiKey: false, reportTemplates: templates }
    const adhoc = localReport({ notes: [], goals: [{ ...goal('2026-10-09', false), title: '临时插入', adHoc: true }], settings }, request)
    expect(adhoc.markdown).toContain('临时插入（插活）')
  })
  it('旧默认模板自动更新，自定义模板和旧变量保持可用', () => {
    const previous = '# {{标题}}\n\n{{范围}}\n\n## 今日概况\n{{概况}}\n\n## 今日完成\n{{已完成待办}}\n\n## 待继续推进\n{{未完成待办}}'
    const custom = '自定义周报\n{{已完成待办}}'
    const progress = '月报{{日期范围}}完成任务进度和计划\n\n【完成情况】：\n{{项目完成情况}}\n\n【遇到问题】\n无\n\n【下月计划】\n{{项目计划}}'
    const templates = resolveReportTemplates({ daily: previous, weekly: custom, monthly: progress })
    expect(templates).toEqual({ ...defaultReportTemplates, weekly: custom })
    for (const [type, kind, date, plan] of [
      ['daily', '日报', '日期年-月-日', '明日计划'],
      ['weekly', '周报', '日期范围', '下周计划'],
      ['monthly', '月报', '日期范围', '下月计划'],
    ] as const) {
      const old = `${kind}{{${date}}}完成任务进度和计划\n\n【完成情况】：\n{{项目完成情况}}\n\n【遇到问题】\n无\n\n【${plan}】\n{{项目计划}}`
      expect(resolveReportTemplates({ [type]: old })[type]).toBe(defaultReportTemplates[type])
    }
    const report = localReport({ notes: [], goals: [goal('2026-10-09', true)], settings: { endpoint: '', model: '', hasApiKey: false, reportTemplates: templates } }, request)
    expect(report.markdown).toContain('自定义周报\n- [x] 完成设计')
  })

})

describe('Git 令牌入口', () => {
  it('按平台生成令牌页面，自建 GitLab 校验地址', () => {
    expect(gitTokenPage({ provider: 'github', gitlabUrl: '' })).toBe('https://github.com/settings/tokens/new?scopes=repo&description=riji-worklog')
    expect(gitTokenPage({ provider: 'gitlab', gitlabUrl: 'https://git.example.com/sub/' })).toBe('https://git.example.com/sub/-/user_settings/personal_access_tokens?name=riji-worklog&scopes=read_api')
    expect(() => gitTokenPage({ provider: 'gitlab', gitlabUrl: 'http://git.example.com' })).toThrow()
    expect(() => gitTokenPage({ provider: 'gitlab', gitlabUrl: 'https://user:pw@git.example.com' })).toThrow()
  })
})
