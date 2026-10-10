<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { ArrowRight, BarChart3, Check, FileText, LoaderCircle, Menu, NotebookPen, Search, Settings2, X } from '@lucide/vue'
import { desktop } from './api'
import { noteSummary, shortDate, type DateRange, type ReportType } from './domain'
import { initialize, loadError, loading, selectedDate, toast, view, workspace } from './store'
import CalendarPanel from './components/CalendarPanel.vue'
import JournalPage from './components/JournalPage.vue'
import ReportCenter from './components/ReportCenter.vue'
import InsightsPage from './components/InsightsPage.vue'
import SettingsModal from './components/SettingsModal.vue'
import WindowControls from './components/WindowControls.vue'

const generatorOpen = ref(false)
const settingsOpen = ref(false)
const mobileNavOpen = ref(false)
const search = ref('')
const searchInput = ref<HTMLInputElement>()
const journal = ref<InstanceType<typeof JournalPage>>()
const reports = ref<InstanceType<typeof ReportCenter>>()
const navItems = [{ value: 'journal', label: '每日记录', icon: NotebookPen }, { value: 'reports', label: '工作报告', icon: FileText }, { value: 'insights', label: '统计概览', icon: BarChart3 }] as const
const title = computed(() => navItems.find(item => item.value === view.value)!.label)
const matches = computed(() => {
  const query = search.value.trim().toLowerCase()
  if (!query) return []
  return [...workspace.value.notes.filter(note => `${noteSummary(note)} ${note.tag}`.toLowerCase().includes(query)).map(note => ({ id: note.id, date: note.date, content: noteSummary(note), type: '手记' })), ...workspace.value.goals.filter(goal => goal.title.toLowerCase().includes(query)).map(goal => ({ id: goal.id, date: goal.date, content: goal.title, type: '待办' }))].sort((a, b) => b.date.localeCompare(a.date)).slice(0, 30)
})
function navigate(next: typeof view.value) { view.value = next; mobileNavOpen.value = false; search.value = '' }
function selectSearch(date: string) { selectedDate.value = date; navigate('journal') }
function openGenerator(type?: ReportType, anchorDate = selectedDate.value, range?: DateRange) { reports.value?.configureRequest(type, anchorDate, range); generatorOpen.value = true }
function openAiSettings() { generatorOpen.value = false; settingsOpen.value = true }
async function keyboard(event: KeyboardEvent) {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); searchInput.value?.focus() }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'n' && !settingsOpen.value && !generatorOpen.value && !document.querySelector('[role="dialog"]')) { event.preventDefault(); view.value = 'journal'; await nextTick(); journal.value?.openEditor() }
  if (event.key === 'Escape') { search.value = ''; mobileNavOpen.value = false }
}
onMounted(() => { initialize(); document.addEventListener('keydown', keyboard) })
onBeforeUnmount(() => document.removeEventListener('keydown', keyboard))
</script>

<template>
  <div class="app-shell"><button v-if="mobileNavOpen" class="nav-backdrop" aria-label="关闭导航" @click="mobileNavOpen = false" /><aside class="sidebar" :class="{ 'mobile-open': mobileNavOpen }"><a class="brand" href="#" @click.prevent="navigate('journal')"><img src="/app-icon.png" alt="" /><strong>日迹</strong></a><nav aria-label="主导航"><button v-for="item in navItems" :key="item.value" class="nav-item" :class="{ active: view === item.value }" :aria-current="view === item.value ? 'page' : undefined" @click="navigate(item.value)"><component :is="item.icon" :size="20" :stroke-width="1.7" /><span>{{ item.label }}</span><span v-if="item.value === 'reports' && workspace.reports.length" class="nav-count">{{ workspace.reports.length }}</span></button></nav><CalendarPanel /><div class="sidebar-bottom"><button class="nav-item settings-nav" @click="settingsOpen = true; mobileNavOpen = false"><Settings2 :size="20" :stroke-width="1.7" /><span>设置</span></button><span v-if="!desktop" class="preview-indicator">浏览器预览</span></div></aside>
    <div class="main-shell"><header class="topbar" :data-tauri-drag-region="desktop ? true : undefined"><div class="topbar-path" :data-tauri-drag-region="desktop ? true : undefined"><button class="icon-button mobile-menu" aria-label="打开导航" @click="mobileNavOpen = true"><Menu :size="20" /></button><strong :data-tauri-drag-region="desktop ? true : undefined">{{ title }}</strong></div><div class="topbar-right"><div class="search-wrap"><Search :size="17" /><input ref="searchInput" v-model="search" aria-label="搜索手记与待办" placeholder="搜索记录" /><button v-if="search" class="icon-button small" aria-label="清空搜索" @click="search = ''"><X :size="14" /></button><kbd v-else>Ctrl K</kbd><div v-if="search.trim()" class="search-results"><div class="search-heading">搜索结果 <span>{{ matches.length }}{{ matches.length === 30 ? '+' : '' }}</span></div><button v-for="result in matches" :key="result.id" @click="selectSearch(result.date)"><span class="search-result-meta">{{ shortDate(result.date, true) }} · {{ result.type }}</span><span>{{ result.content }}</span><ArrowRight :size="14" /></button><p v-if="!matches.length">没有找到相关记录</p></div></div></div><WindowControls v-if="desktop" /></header>
      <main class="main-content"><div v-if="loading" class="loading-state"><LoaderCircle class="spin" :size="23" /><span>正在读取工作空间…</span></div><div v-else-if="loadError" class="empty-state"><h3>暂时无法读取数据</h3><p class="inline-error">{{ loadError }}</p><button class="button primary" @click="initialize">重新读取</button></div><template v-else><div v-show="view === 'journal'" class="page"><JournalPage ref="journal" @generate="openGenerator" /></div><div v-show="view === 'reports'" class="page"><ReportCenter ref="reports" :generator-open="generatorOpen" @open-generator="openGenerator()" @close-generator="generatorOpen = false" @settings="openAiSettings" /></div><div v-if="view === 'insights'" class="page"><InsightsPage v-if="view === 'insights'" @generate="(type, anchorDate) => openGenerator(type, anchorDate)" /></div></template></main>
    </div><SettingsModal v-if="settingsOpen" @close="settingsOpen = false" /><Transition name="toast"><div v-if="toast" class="toast-message" role="status"><Check :size="16" />{{ toast }}</div></Transition></div>
</template>
