import { computed, ref } from 'vue'
import { api } from './api'
import { dateKey, statisticsFor, type Workspace } from './domain'

export const workspace = ref<Workspace>({ notes: [], goals: [], reports: [], settings: { endpoint: 'https://api.openai.com/v1', model: '', hasApiKey: false }, storagePath: '' })
export const selectedDate = ref(dateKey(new Date()))
export const view = ref<'journal' | 'reports' | 'insights' | 'plans'>('journal')
export const loading = ref(true)
export const busy = ref(false)
export const loadError = ref('')
export const toast = ref('')
export const modalOpen = ref(false)
let toastTimer: ReturnType<typeof setTimeout>

export function notify(message: string) {
  toast.value = message
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => { toast.value = '' }, 4500)
}

export async function refresh() {
  workspace.value = await api.load()
}

export async function initialize() {
  loading.value = true
  loadError.value = ''
  try { await refresh() } catch (error) { loadError.value = String(error) }
  finally { loading.value = false }
}

export async function mutate(action: () => Promise<unknown>, message = ''): Promise<boolean> {
  if (busy.value) return false
  busy.value = true
  try {
    await action()
    await refresh()
    if (message) notify(message)
    return true
  } catch (error) { notify(error instanceof Error ? error.message : String(error)); return false }
  finally { busy.value = false }
}

export const dateNotes = computed(() => workspace.value.notes.filter(note => note.date === selectedDate.value).sort((a, b) => b.createdAt.localeCompare(a.createdAt)))
export const dateGoals = computed(() => workspace.value.goals.filter(goal => goal.date === selectedDate.value).sort((a, b) => a.createdAt.localeCompare(b.createdAt)))
export const dateStatistics = computed(() => statisticsFor(dateNotes.value, dateGoals.value))
