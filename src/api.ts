import { invoke, isTauri } from '@tauri-apps/api/core'
import { dateKey, maxMinutes, defaultReportTemplates, resolveReportTemplates, validateReportTemplates, localReport, defaultGitSettings, gitTokenPage, type AiSettings, type GitSettings, type Goal, type ImageUpload, type Note, type NoteImage, type Report, type ReportRequest, type Workspace } from './domain'
import { maxImageBytes, prunePreviewImages, readPreviewImage, writePreviewImages, type StoredImage } from './images'

export const desktop = isTauri()
const storageKey = 'riji-preview-v1'

function blankWorkspace(): Workspace {
  return { notes: [], goals: [], reports: [], settings: { endpoint: 'https://api.openai.com/v1', model: '', hasApiKey: false, reportTemplates: { ...defaultReportTemplates } }, storagePath: '当前浏览器 · 预览数据' }
}

function readPreview(): Workspace {
  const raw = localStorage.getItem(storageKey)
  if (!raw) return blankWorkspace()
  try {
    const value = JSON.parse(raw) as Workspace
    if (!Array.isArray(value.notes) || !Array.isArray(value.goals) || !Array.isArray(value.reports) || !value.settings) throw new Error('invalid')
    return { ...value, settings: { ...value.settings, reportTemplates: resolveReportTemplates(value.settings.reportTemplates) }, storagePath: '当前浏览器 · 预览数据' }
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

export function validateGoal(goal: Goal) {
  if (!goal.id || !goal.title?.trim() || goal.title.length > 300 || typeof goal.completed !== 'boolean') throw new Error('待办需为 1 至 300 字')
  if (goal.project !== undefined && (goal.project.length > 40 || goal.project.includes('\n'))) throw new Error('项目名称不能超过 40 字，且需在一行内')
  for (const minutes of [goal.estimateMin, goal.actualMin]) {
    if (minutes !== undefined && (!Number.isInteger(minutes) || minutes < 0 || minutes > maxMinutes)) throw new Error('耗时需在 0 至 100000 分钟之间')
  }
  if (goal.completedAt && (!goal.completed || Number.isNaN(Date.parse(goal.completedAt)))) throw new Error('完成时间不正确')
  if (goal.originDate !== undefined && !/^\d{4}-\d{2}-\d{2}$/.test(goal.originDate)) throw new Error('首次计划日期不正确')
  if (goal.priority !== undefined && !['high', 'normal', 'low'].includes(goal.priority)) throw new Error('优先级不正确')
  if (goal.risk !== undefined && (goal.risk.length > 300 || goal.risk.includes('\n'))) throw new Error('风险／原因不能超过 300 字，且需在一行内')
  if (goal.adHoc !== undefined && typeof goal.adHoc !== 'boolean') throw new Error('插活标记不正确')
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
  async saveGoals(goals: Goal[]): Promise<void> {
    if (!goals.length || goals.length > 500) throw new Error('一次最多处理 500 条待办')
    goals.forEach(validateGoal)
    if (desktop) return invoke('save_goals', { goals })
    const workspace = readPreview()
    for (const goal of goals) {
      const index = workspace.goals.findIndex(item => item.id === goal.id)
      if (index < 0) workspace.goals.push(goal)
      else workspace.goals[index] = goal
    }
    writePreview(workspace)
  },
  async exportCsv(name: string, content: string): Promise<boolean> {
    if (desktop) return invoke('export_csv', { name, content })
    download(name, content, 'text/csv;charset=utf-8')
    return true
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
  async saveSettings(settings: AiSettings, apiKey: string, clearKey: boolean, gitToken = '', clearGitToken = false): Promise<void> {
    validateReportTemplates({ ...defaultReportTemplates, ...settings.reportTemplates })
    if (desktop) return invoke('save_ai_settings', { settings, apiKey, clearKey, gitToken, clearGitToken })
    const workspace = readPreview()
    workspace.settings = { ...settings, hasApiKey: false, git: { ...defaultGitSettings, ...settings.git, hasToken: false } }
    writePreview(workspace)
  },
  async testGit(git: GitSettings, token: string): Promise<string> {
    if (!desktop) throw new Error('Git 连接测试需要在桌面应用中运行')
    return invoke('test_git_connection', { git, token })
  },
  async openGitTokenPage(git: GitSettings): Promise<void> {
    if (desktop) return invoke('open_git_token_page', { git })
    window.open(gitTokenPage(git), '_blank', 'noopener,noreferrer')
  },
  async testAi(): Promise<string> {
    if (!desktop) throw new Error('连接测试需要在桌面应用中运行')
    return invoke('test_ai_connection')
  },
  async listModels(endpoint: string, apiKey: string, clearKey: boolean): Promise<string[]> {
    if (!desktop) throw new Error('模型刷新需要在桌面应用中运行')
    return invoke('list_ai_models', { endpoint, apiKey, clearKey })
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
    download(`日迹备份-${dateKey(new Date())}.json`, JSON.stringify({ version: 2, notes: workspace.notes, goals: workspace.goals, reports: workspace.reports, images, reportTemplates: workspace.settings.reportTemplates }, null, 2), 'application/json')
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
          if (backup.reportTemplates) validateReportTemplates(backup.reportTemplates)
          const validNote = (note: Note) => note.id && typeof note.content === 'string' && (note.content.trim() || note.images?.length) && (!note.images || (Array.isArray(note.images) && note.images.length <= 9)) && /^\d{4}-\d{2}-\d{2}$/.test(note.date) && ['note', 'progress', 'blocker'].includes(note.kind)
          if (notes.some(note => !validNote(note)) || goals.some(goal => { try { validateGoal(goal); return false } catch { return true } }) || reports.some(report => !report.id || typeof report.markdown !== 'string' || !report.statistics || report.sources?.notes?.some(note => !validNote(note)))) throw new Error('备份内容不完整')
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
          if (backup.reportTemplates) workspace.settings.reportTemplates = { ...backup.reportTemplates }
          writePreview(workspace)
          await prunePreviewImages(new Set(imageReferences(workspace).keys()))
          resolve(true)
        } catch (error) { reject(error) }
      }
      input.click()
    })
  },
}
