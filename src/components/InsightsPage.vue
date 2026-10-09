<script setup lang="ts">
import { computed, ref } from 'vue'
import { ArrowUpRight, CalendarDays, CircleCheck, NotebookPen, Sparkles } from '@lucide/vue'
import { addDays, dateKey, kindNames, periodFor, statisticsFor } from '../domain'
import { selectedDate, view, workspace } from '../store'
const emit = defineEmits<{ generate: [] }>()
const month = ref(selectedDate.value.slice(0, 7))
const period = computed(() => { try { return periodFor('monthly', `${month.value}-01`) } catch { return periodFor('monthly', dateKey(new Date())) } })
const notes = computed(() => workspace.value.notes.filter(note => note.date >= period.value.start && note.date <= period.value.end))
const goals = computed(() => workspace.value.goals.filter(goal => goal.date >= period.value.start && goal.date <= period.value.end))
const stats = computed(() => statisticsFor(notes.value, goals.value))
const days = computed(() => Array.from({ length: Number(period.value.end.slice(8)) }, (_, index) => { const date = addDays(period.value.start, index); return { date, notes: notes.value.filter(note => note.date === date).length, label: index + 1 } }))
const maxNotes = computed(() => Math.max(4, Math.ceil(Math.max(0, ...days.value.map(day => day.notes)) / 2) * 2))
const tags = computed(() => {
  const counts = new Map<string, number>()
  notes.value.forEach(note => counts.set(note.tag || '未分类', (counts.get(note.tag || '未分类') || 0) + 1))
  return [...counts.entries()].sort((a, b) => b[1] - a[1])
})
const maxTags = computed(() => Math.max(1, ...tags.value.map(item => item[1])))
function visit(date: string) { selectedDate.value = date; view.value = 'journal' }
</script>

<template>
  <div class="page-heading"><h1>统计概览</h1><input v-model="month" aria-label="统计月份" class="month-input" type="month" min="1900-01" max="9999-12" /></div><div class="insight-metrics"><div class="metric-card"><CalendarDays :size="20" /><span>记录天数</span><strong>{{ stats.days }}<small>天</small></strong></div><div class="metric-card"><NotebookPen :size="20" /><span>工作手记</span><strong>{{ stats.notes }}<small>条</small></strong></div><div class="metric-card"><CircleCheck :size="20" /><span>待办完成</span><strong>{{ stats.completed }}<small>/ {{ stats.goals }}</small></strong><span class="metric-foot">{{ stats.goals ? `${stats.completionRate}% 完成率` : '暂无待办' }}</span></div></div>
  <section class="panel activity-panel"><div class="section-heading"><h2>记录节奏</h2><span class="subtle">{{ month }} · 每日手记数量</span></div><div class="activity-chart"><div class="chart-grid"><span>{{ maxNotes }}</span><span>{{ Math.round(maxNotes / 2) }}</span><span>0</span></div><div class="activity-bars"><button v-for="day in days" :key="day.date" :title="`${day.date}：${day.notes} 条手记`" :aria-label="`${day.date}：${day.notes} 条手记，查看当日`" @click="visit(day.date)"><div class="activity-bar-track"><i :style="{ height: `${day.notes / maxNotes * 100}%` }" :class="{ empty: day.notes === 0 }" /></div><span>{{ day.label === 1 || (day.label % 5 === 0 && day.label < days.length - 1) || day.label === days.length ? day.label : '' }}</span></button></div></div></section>
  <div class="insight-lower"><section class="panel"><div class="section-heading"><h2>工作分布</h2><span class="subtle">按标签统计</span></div><div v-if="tags.length" class="tag-chart"><div v-for="[tag, count] in tags.slice(0, 8)" :key="tag"><span>{{ tag }}</span><div class="tag-track"><i :style="{ width: `${count / maxTags * 100}%` }" /></div><strong>{{ count }}</strong></div><p v-if="tags.length > 8" class="subtle">另有 {{ tags.length - 8 }} 个标签</p></div><p v-else class="empty-chart">暂无记录</p></section><section class="panel note-distribution"><div class="section-heading"><h2>手记分类</h2></div><div v-for="kind in (['progress', 'note', 'blocker'] as const)" :key="kind" class="kind-summary"><i :class="kind" /><span>{{ kindNames[kind] }}</span><strong>{{ notes.filter(note => note.kind === kind).length }}</strong><span class="subtle">条</span></div><button class="insight-report-link" @click="selectedDate = period.start; emit('generate')"><Sparkles :size="16" />生成这个月的报告<ArrowUpRight :size="17" /></button></section></div>
</template>
