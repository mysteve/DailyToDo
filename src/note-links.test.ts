import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from './api'
import type { Goal, Note } from './domain'

vi.mock('./images', () => ({ maxImageBytes: 2 * 1024 * 1024, writePreviewImages: vi.fn(), readPreviewImage: vi.fn(), prunePreviewImages: vi.fn() }))

const now = '2026-10-12T09:00:00+08:00'
const goal: Goal = { id: crypto.randomUUID(), date: '2026-10-12', title: '完成页面调整', completed: false, createdAt: now, updatedAt: now }
const note: Note = { id: crypto.randomUUID(), date: '2026-10-11', content: '已完成布局草案', kind: 'progress', tag: '', createdAt: now, updatedAt: now, goalId: goal.id }

beforeEach(() => {
  const values = new Map<string, string>()
  vi.stubGlobal('localStorage', { getItem: (key: string) => values.get(key) ?? null, setItem: (key: string, value: string) => values.set(key, value) })
})

describe('工作记录关联待办', () => {
  it('跨日期保存关联，待办改名和顺延后仍保留，允许解除关联', async () => {
    await api.saveGoal(goal)
    await api.saveNote(note)
    await api.saveGoal({ ...goal, title: '更新后的待办', date: '2026-10-13' })
    expect((await api.load()).notes[0]?.goalId).toBe(goal.id)
    const { goalId: _, ...unlinked } = note
    await api.saveNote(unlinked)
    expect((await api.load()).notes).toHaveLength(1)
    expect((await api.load()).notes[0]?.goalId).toBeUndefined()
  })

  it('删除待办只解除关联，保留记录正文与时间', async () => {
    await api.saveGoal(goal)
    await api.saveNote(note)
    await api.deleteGoal(goal.id)
    const workspace = await api.load()
    expect(workspace.goals).toHaveLength(0)
    expect(workspace.notes[0]).toEqual(expect.objectContaining({ content: note.content, date: note.date, createdAt: note.createdAt }))
    expect(workspace.notes[0]?.goalId).toBeUndefined()
  })

  it('不存在的待办拒绝关联，旧记录仍能保存', async () => {
    await expect(api.saveNote(note)).rejects.toThrow('关联待办已不存在')
    expect((await api.load()).notes).toHaveLength(0)
    const { goalId: _, ...legacy } = note
    await api.saveNote(legacy)
    expect((await api.load()).notes[0]?.content).toBe(note.content)
  })
})
