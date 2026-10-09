import { invoke, isTauri } from '@tauri-apps/api/core'
import { dateKey, addDays, localReport, type AiSettings, type Goal, type ImageUpload, type Note, type NoteImage, type Report, type ReportRequest, type Workspace } from './domain'
import { maxImageBytes, prunePreviewImages, readPreviewImage, writePreviewImages, type StoredImage } from './images'

export const desktop = isTauri()
const storageKey = 'riji-preview-v1'

function blankWorkspace(): Workspace {
  return { notes: [], goals: [], reports: [], settings: { endpoint: 'https://api.openai.com/v1', model: '', hasApiKey: false }, storagePath: '当前浏览器 · 预览数据' }
}

function readPreview(): Workspace {
  const raw = localStorage.getItem(storageKey)
  if (!raw) return blankWorkspace()
  try {
    const value = JSON.parse(raw) as Workspace
    if (!Array.isArray(value.notes) || !Array.isArray(value.goals) || !Array.isArray(value.reports) || !value.settings) throw new Error('invalid')
    return { ...value, storagePath: '当前浏览器 · 预览数据' }
  } catch {
    throw new Error('预览数据无法读取。请先导出浏览器存储中的原始数据，再清理该站点数据。')
  }
}

function writePreview(workspace: Workspace) {
  localStorage.setItem(storageKey, JSON.stringify(workspace))
}

function imageReferences(workspace: Pick<Workspace, 'notes' | 'reports'>) {
  const notes = [...workspace.notes, ...workspace.reports.flatMap(report => report.sources?.notes || [])]
  return new Map(notes.flatMap(note => note.images || []).map(image => [image.id, image]))
}

function validateStoredImage(asset: StoredImage) {
  const image = asset.image
  if (!image?.id || !image.name || image.name.length > 200 || !['image/png', 'image/jpeg', 'image/webp'].includes(image.mime) || !Number.isInteger(image.width) || !Number.isInteger(image.height) || image.width < 1 || image.height < 1 || image.width > 2048 || image.height > 2048 || typeof asset.data !== 'string' || asset.data.length > Math.ceil(maxImageBytes / 3) * 4) throw new Error('图片格式不正确')
  const bytes = atob(asset.data)
  const valid = image.mime === 'image/png' ? bytes.startsWith('\x89PNG\r\n\x1a\n') : image.mime === 'image/jpeg' ? bytes.startsWith('\xff\xd8\xff') : bytes.startsWith('RIFF') && bytes.slice(8, 12) === 'WEBP'
  if (!valid || bytes.length > maxImageBytes) throw new Error('图片数据不正确或超过 2 MB')
}

function download(filename: string, content: string, mime: string) {
  const url = URL.createObjectURL(new Blob([content], { type: mime }))
  const link = document.createElement('a')
  link.href = url
  link.download = filename
  link.click()
  setTimeout(() => URL.revokeObjectURL(url), 1000)
}

export const api = {
  async load(): Promise<Workspace> {
    return desktop ? invoke('load_workspace') : readPreview()
  },
  async saveNote(note: Note, images: ImageUpload[] = []): Promise<void> {
    if (desktop) return invoke('save_note', { note, images })
    const workspace = readPreview()
    const assets = images.map(upload => {
      const image = note.images?.find(image => image.id === upload.id)
      if (!image) throw new Error('上传图片未关联记录')
      const asset = { image, data: upload.data }
      validateStoredImage(asset)
      return asset
    })
    await writePreviewImages(assets)
    for (const image of note.images || []) await readPreviewImage(image.id)
    const index = workspace.notes.findIndex(item => item.id === note.id)
    if (index < 0) workspace.notes.push(note)
    else workspace.notes[index] = note
    writePreview(workspace)
    await prunePreviewImages(new Set(imageReferences(workspace).keys()))
  },
  async readImage(image: NoteImage): Promise<string> {
    if (desktop) return invoke('read_note_image', { id: image.id })
    const asset = await readPreviewImage(image.id)
    return `data:${asset.image.mime};base64,${asset.data}`
  },
  async deleteNote(id: string): Promise<void> {
    if (desktop) return invoke('delete_note', { id })
    const workspace = readPreview()
    workspace.notes = workspace.notes.filter(note => note.id !== id)
    writePreview(workspace)
    await prunePreviewImages(new Set(imageReferences(workspace).keys()))
  },
  async saveGoal(goal: Goal): Promise<void> {
    if (desktop) return invoke('save_goal', { goal })
    const workspace = readPreview()
    const index = workspace.goals.findIndex(item => item.id === goal.id)
    if (index < 0) workspace.goals.push(goal)
    else workspace.goals[index] = goal
    writePreview(workspace)
  },
  async deleteGoal(id: string): Promise<void> {
    if (desktop) return invoke('delete_goal', { id })
    const workspace = readPreview()
    workspace.goals = workspace.goals.filter(goal => goal.id !== id)
    writePreview(workspace)
  },
  async generate(request: ReportRequest): Promise<Report> {
    if (desktop) return invoke('generate_report', { request })
    if (request.mode === 'ai') throw new Error('请在桌面应用中配置并使用 AI 服务')
    const workspace = readPreview()
    const report = localReport(workspace, request)
    workspace.reports.unshift(report)
    writePreview(workspace)
    return report
  },
  async deleteReport(id: string): Promise<void> {
    if (desktop) return invoke('delete_report', { id })
    const workspace = readPreview()
    workspace.reports = workspace.reports.filter(report => report.id !== id)
    writePreview(workspace)
    await prunePreviewImages(new Set(imageReferences(workspace).keys()))
  },
  async saveSettings(settings: AiSettings, apiKey: string, clearKey: boolean): Promise<void> {
    if (desktop) return invoke('save_ai_settings', { settings, apiKey, clearKey })
    const workspace = readPreview()
    workspace.settings = { ...settings, hasApiKey: false }
    writePreview(workspace)
  },
  async testAi(): Promise<string> {
    if (!desktop) throw new Error('连接测试需要在桌面应用中运行')
    return invoke('test_ai_connection')
  },
  async exportReport(report: Report): Promise<boolean> {
    if (desktop) return invoke('export_report', { id: report.id })
    download(`${report.title}.md`, report.markdown, 'text/markdown;charset=utf-8')
    return true
  },
  async exportBackup(): Promise<boolean> {
    if (desktop) return invoke('export_backup')
    const workspace = readPreview()
    const images = await Promise.all([...imageReferences(workspace).keys()].map(readPreviewImage))
    download(`日迹备份-${dateKey(new Date())}.json`, JSON.stringify({ version: 2, notes: workspace.notes, goals: workspace.goals, reports: workspace.reports, images }, null, 2), 'application/json')
    return true
  },
  async importBackup(): Promise<boolean> {
    if (desktop) return invoke('import_backup')
    return new Promise((resolve, reject) => {
      const input = document.createElement('input')
      input.type = 'file'
      input.accept = '.json'
      input.oncancel = () => resolve(false)
      input.onchange = async () => {
        try {
          const file = input.files?.[0]
          if (!file) return resolve(false)
          if (file.size > 256 * 1024 * 1024) throw new Error('备份文件不能超过 256 MB')
          const backup = JSON.parse(await file.text())
          if (![1, 2].includes(backup.version) || !Array.isArray(backup.notes) || !Array.isArray(backup.goals) || !Array.isArray(backup.reports)) throw new Error('备份格式不正确')
          const workspace = readPreview()
          const merge = <T extends { id: string }>(existing: T[], incoming: T[]): T[] => {
            const ids = new Set(existing.map(item => item.id))
            return [...existing, ...incoming.filter(item => !ids.has(item.id))]
          }
          const notes = backup.notes as Note[]
          const goals = backup.goals as Goal[]
          const reports = backup.reports as Report[]
          const validNote = (note: Note) => note.id && typeof note.content === 'string' && (note.content.trim() || note.images?.length) && (!note.images || (Array.isArray(note.images) && note.images.length <= 9)) && /^\d{4}-\d{2}-\d{2}$/.test(note.date) && ['note', 'progress', 'blocker'].includes(note.kind)
          if (notes.some(note => !validNote(note)) || goals.some(goal => !goal.id || !goal.title || typeof goal.completed !== 'boolean') || reports.some(report => !report.id || typeof report.markdown !== 'string' || !report.statistics || report.sources?.notes?.some(note => !validNote(note)))) throw new Error('备份内容不完整')
          const images: StoredImage[] = backup.images || []
          if (!Array.isArray(images) || new Set(images.map(asset => asset.image?.id)).size !== images.length) throw new Error('备份图片格式不正确')
          images.forEach(validateStoredImage)
          await writePreviewImages(images)
          for (const image of imageReferences({ notes, reports }).values()) {
            const asset = await readPreviewImage(image.id)
            if ((['id', 'name', 'mime', 'width', 'height'] as const).some(key => asset.image[key] !== image[key])) throw new Error('备份图片信息不一致')
          }
          workspace.notes = merge(workspace.notes, notes)
          workspace.goals = merge(workspace.goals, goals)
          workspace.reports = merge(workspace.reports, reports)
          writePreview(workspace)
          await prunePreviewImages(new Set(imageReferences(workspace).keys()))
          resolve(true)
        } catch (error) { reject(error) }
      }
      input.click()
    })
  },
  async loadExamples(): Promise<void> {
    const workspace = await this.load()
    if (workspace.notes.length || workspace.goals.length) throw new Error('已有工作记录，无法载入示例')
    const today = dateKey(new Date())
    const at = (date: string, hour: string) => new Date(`${date}T${hour}:00`).toISOString()
    const rows: [number, string, Note['kind'], string, string][] = [
      [0, '14:30', 'progress', '产品设计', '完成工作记录页与报告生成流程的设计。梳理了按日期记录、目标追踪和按需汇报三个主要场景。'],
      [0, '11:20', 'note', '团队协作', '与团队确认首版范围：优先做好本地记录和报告，后续再评估跨设备同步。'],
      [0, '09:15', 'blocker', '开发', 'AI 服务的超时与重试策略还需要确认，安排下一次技术讨论。'],
      [-1, '16:40', 'progress', '产品设计', '完成桌面端信息结构，整理日报、周报和月报的内容模板。'],
      [-2, '10:30', 'progress', '开发', '验证本地数据存储方案，明确备忘录与目标的数据结构。'],
      [-3, '15:00', 'note', '调研', '整理工作留痕需求：快速记录、按日回顾、手动选择报告类型。'],
      [-7, '14:00', 'progress', '调研', '完成首轮需求访谈，记录常见工作汇报场景。'],
    ]
    const notes: Note[] = rows.map(([offset, hour, kind, tag, content]) => ({ id: crypto.randomUUID(), date: addDays(today, offset), content, kind, tag, createdAt: at(addDays(today, offset), hour), updatedAt: at(addDays(today, offset), hour) }))
    const goalRows: [number, string, boolean][] = [[0, '完成工作记录页设计', true], [0, '梳理报告生成流程', true], [0, '确认 AI 接口与异常处理', false], [0, '整理本周工作周报', false], [-1, '确定桌面端信息结构', true], [-2, '验证本地数据存储', true], [-3, '整理用户需求', true], [-7, '完成首轮需求访谈', true]]
    const goals: Goal[] = goalRows.map(([offset, title, completed]) => ({ id: crypto.randomUUID(), date: addDays(today, offset), title, completed, createdAt: at(addDays(today, offset), '09:00'), updatedAt: at(addDays(today, offset), '09:00') }))
    if (desktop) return invoke('import_examples', { notes, goals })
    writePreview({ ...workspace, notes, goals })
  },
}
