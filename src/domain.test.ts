import { describe, expect, it } from 'vitest'
import { localReport, periodFor, requestPeriod, selectSources, statisticsFor, type Goal, type Note, type ReportRequest } from './domain'

const request: ReportRequest = { type: 'weekly', anchorDate: '2026-10-09', style: 'brief', mode: 'local', includeNotes: true, includeGoals: true, instruction: '' }
const note = (date: string, kind: Note['kind'] = 'progress'): Note => ({ id: crypto.randomUUID(), date, kind, content: '完成页面设计', tag: '设计', createdAt: `${date}T09:00:00+08:00`, updatedAt: `${date}T09:00:00+08:00` })
const goal = (date: string, completed: boolean): Goal => ({ id: crypto.randomUUID(), date, title: completed ? '完成设计' : '待确认接口', completed, createdAt: `${date}T09:00:00+08:00`, updatedAt: `${date}T09:00:00+08:00` })

describe('按用户选择的周期整理报告', () => {
  it('周从周一开始，跨年周与闰月计算准确', () => {
    expect(periodFor('weekly', '2027-01-01')).toEqual({ start: '2026-12-28', end: '2027-01-03' })
    expect(periodFor('weekly', '2026-10-11')).toEqual({ start: '2026-10-05', end: '2026-10-11' })
    expect(periodFor('monthly', '2024-02-15')).toEqual({ start: '2024-02-01', end: '2024-02-29' })
    expect(() => periodFor('daily', '2026-02-30')).toThrow()
  })
  it('仅包含日期范围内的所选来源', () => {
    const data = { notes: [note('2026-10-04'), note('2026-10-05'), note('2026-10-11'), note('2026-10-12')], goals: [goal('2026-10-09', true)] }
    expect(selectSources(data, request).notes).toHaveLength(2)
    expect(selectSources(data, { ...request, includeNotes: false }).notes).toHaveLength(0)
    expect(selectSources(data, { ...request, includeGoals: false }).goals).toHaveLength(0)
  })
  it('自选范围保留跨月、跨年边界，并包含起止日', () => {
    const data = { notes: [note('2026-09-29'), note('2026-09-30'), note('2026-10-02'), note('2026-10-03')], goals: [goal('2026-09-30', true), goal('2026-10-02', false), goal('2026-10-03', true)] }
    for (const type of ['weekly', 'monthly'] as const) {
      const custom = { ...request, type, startDate: '2026-09-30', endDate: '2026-10-02' }
      const report = localReport(data, custom)
      expect([report.startDate, report.endDate]).toEqual(['2026-09-30', '2026-10-02'])
      expect(report.sources.notes.map(item => item.date)).toEqual(['2026-09-30', '2026-10-02'])
      expect(report.statistics).toMatchObject({ days: 2, notes: 2, goals: 2, completed: 1, completionRate: 50 })
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
    expect(stats).toEqual({ days: 2, notes: 2, goals: 3, completed: 1, blockers: 1, completionRate: 33 })
    expect(statisticsFor([], []).completionRate).toBe(0)
  })
  it('保存真实来源，并区分已完成和未完成目标', () => {
    const data = { notes: [note('2026-10-09')], goals: [goal('2026-10-09', true), goal('2026-10-09', false)] }
    const report = localReport(data, request)
    expect(report.markdown).toContain('- [x] 完成设计')
    expect(report.markdown).toContain('- [ ] 待确认接口')
    expect(report.sources.notes).toHaveLength(1)
    expect(report.mode).toBe('local')
  })
  it('空范围不保存报告，详细汇报按日展开', () => {
    expect(() => localReport({ notes: [], goals: [] }, request)).toThrow('没有可用于生成')
    const report = localReport({ notes: [note('2026-10-09')], goals: [] }, { ...request, style: 'detailed' })
    expect(report.markdown).toContain('### 10月9日')
  })
  it('纯图片记录保留附件来源，各种汇报仅标注图片数量', () => {
    const imageNote = { ...note('2026-10-09'), content: '', images: [{ id: crypto.randomUUID(), name: '截图.png', mime: 'image/png', width: 1200, height: 800 }] }
    for (const style of ['brief', 'detailed', 'review'] as const) {
      const report = localReport({ notes: [imageNote], goals: [] }, { ...request, style })
      expect(report.markdown).toContain('附图 1 张')
      expect(report.statistics.notes).toBe(1)
      expect(report.sources.notes[0]?.images).toEqual(imageNote.images)
      expect(report.markdown).not.toContain('base64')
    }
  })
})
