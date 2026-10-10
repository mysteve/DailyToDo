<script setup lang="ts">
import { computed, ref } from 'vue'
import { AlarmClock, ArrowUpRight, CalendarDays, ChevronLeft, ChevronRight, CircleCheck, Clock, Download, Flame, Sparkles } from '@lucide/vue'
import { api } from '../api'
import { backlog, delta, goalsCsv, heatmap, inRange, insightNames, projectShares, rangeFor, shiftRange, streak, summarize, trend, type InsightKind } from '../analytics'
import { dateKey, formatMinutes, kindNames, parseDate, shortDate } from '../domain'
import { notify, selectedDate, view, workspace } from '../store'
const emit = defineEmits<{ generate: [type: 'weekly' | 'monthly', anchorDate: string] }>()
const kind = ref<InsightKind>('month')
const anchor = ref(selectedDate.value)
const today = dateKey(new Date())
const range = computed(() => rangeFor(kind.value, anchor.value))
const previousRange = computed(() => rangeFor(kind.value, shiftRange(kind.value, anchor.value, -1)))
const notes = computed(() => inRange(workspace.value.notes, range.value))
const goals = computed(() => inRange(workspace.value.goals, range.value))
const summary = computed(() => summarize(goals.value, notes.value))
const previous = computed(() => summarize(inRange(workspace.value.goals, previousRange.value), inRange(workspace.value.notes, previousRange.value)))
const label = computed(() => {
  const { start, end } = range.value
  if (kind.value === 'year') return `${start.slice(0, 4)} 年`
  if (kind.value === 'month') return `${start.slice(0, 4)} 年 ${Number(start.slice(5, 7))} 月`
  return `${shortDate(start, start.slice(0, 4) !== end.slice(0, 4))} — ${shortDate(end, start.slice(0, 4) !== end.slice(0, 4))}`
})
const unit = computed(() => insightNames[kind.value])
const completedDelta = computed(() => delta(summary.value.completed, previous.value.completed, ' 项'))
const timeDelta = computed(() => delta(summary.value.actualMin, previous.value.actualMin, ' 分钟'))
// 持平或没有数据时不显示说明行，避免出现无信息的占位文案
const completedFoot = computed(() => [summary.value.total ? `${summary.value.rate}% 完成率` : '', completedDelta.value.direction === 'flat' ? '' : completedDelta.value.text].filter(Boolean).join(' · '))
const timeFoot = computed(() => timeDelta.value.direction === 'flat' ? '' : timeDelta.value.text)
const buckets = computed(() => trend(goals.value, range.value, kind.value))
const maxBucket = computed(() => Math.max(2, ...buckets.value.flatMap(item => [item.planned, item.completed])))
const maxBucketTime = computed(() => Math.max(1, ...buckets.value.map(item => item.actualMin)))
const showTimeTrend = computed(() => summary.value.actualMin > 0)
const shares = computed(() => projectShares(goals.value))
const maxShare = computed(() => Math.max(1, ...shares.value.map(item => summary.value.actualMin ? item.actualMin : item.total)))
const byTime = computed(() => summary.value.actualMin > 0)
const days = computed(() => heatmap(goals.value, notes.value, range.value))
const heatOffset = computed(() => (parseDate(range.value.start).getDay() + 6) % 7)
// 年视图按周分列并横向铺满；周、月视图按周一至周日排成日历
const heatStyle = computed(() => kind.value === 'year' ? { '--cols': Math.ceil((heatOffset.value + days.value.length) / 7) } : undefined)
const overdue = computed(() => backlog(workspace.value.goals, today))
const currentStreak = computed(() => streak(workspace.value.goals, workspace.value.notes, today))
const tags = computed(() => {
  const counts = new Map<string, number>()
  notes.value.forEach(note => counts.set(note.tag || '未分类', (counts.get(note.tag || '未分类') || 0) + 1))
  return [...counts.entries()].sort((a, b) => b[1] - a[1])
})
const maxTags = computed(() => Math.max(1, ...tags.value.map(item => item[1])))
const accuracyText = computed(() => {
  const accuracy = summary.value.accuracy
  if (!accuracy) return ''
  const percent = Math.round(accuracy.ratio * 100)
  return `${accuracy.count} 项已完成待办，实际为预估的 ${percent}%${percent > 115 ? '，整体偏乐观' : percent < 85 ? '，整体偏保守' : '，估得比较准'}`
})
function step(amount: number) {
  const next = shiftRange(kind.value, anchor.value, amount)
  if (next < '1900-01-01' || next > '9999-12-31') return notify('日期必须在 1900 至 9999 年之间')
  anchor.value = next
}
function visit(date: string) { selectedDate.value = date; view.value = 'journal' }
function ageText(days: number) { return days <= 0 ? '今天计划' : `拖了 ${days} 天` }
async function exportCsv() {
  try {
    if (await api.exportCsv(`日迹-待办-${range.value.start}_${range.value.end}.csv`, goalsCsv(goals.value))) notify('待办明细已导出')
  } catch (error) { notify(error instanceof Error ? error.message : String(error)) }
}
function generate() { if (kind.value !== 'year') emit('generate', kind.value === 'week' ? 'weekly' : 'monthly', range.value.start) }
</script>

<template>
  <div class="page-heading insights-heading">
    <h1>统计概览</h1>
    <div class="insight-controls">
      <div class="segmented" aria-label="统计周期"><button v-for="item in (['week', 'month', 'year'] as const)" :key="item" :class="{ active: kind === item }" :aria-pressed="kind === item" @click="kind = item">{{ insightNames[item] }}</button></div>
      <div class="insight-period"><button class="icon-button small" :aria-label="`前一${unit}`" @click="step(-1)"><ChevronLeft :size="16" /></button><strong>{{ label }}</strong><button class="icon-button small" :aria-label="`后一${unit}`" @click="step(1)"><ChevronRight :size="16" /></button><button class="text-button" @click="anchor = today">本{{ unit }}</button></div>
    </div>
  </div>
  <div class="insights-scroll">
  <div class="insight-metrics five">
    <div class="metric-card"><CalendarDays :size="20" /><span>记录天数</span><strong>{{ summary.recordedDays }}<small>天</small></strong></div>
    <div class="metric-card"><CircleCheck :size="20" /><span>待办完成</span><strong>{{ summary.completed }}<small>/ {{ summary.total }}</small></strong><span v-if="completedFoot" class="metric-foot">{{ completedFoot }}</span></div>
    <div class="metric-card"><Clock :size="20" /><span>实际耗时</span><strong class="time-value">{{ summary.actualMin ? formatMinutes(summary.actualMin) : '未记录' }}</strong><span v-if="timeFoot" class="metric-foot">{{ timeFoot }}</span></div>
    <div class="metric-card"><AlarmClock :size="20" /><span>预估准确度</span><strong class="time-value">{{ summary.accuracy ? `${Math.round(summary.accuracy.ratio * 100)}%` : '—' }}</strong><span v-if="accuracyText" class="metric-foot">{{ accuracyText }}</span></div>
    <div class="metric-card"><Flame :size="20" /><span>连续记录</span><strong>{{ currentStreak }}<small>天</small></strong></div>
  </div>
  <section class="panel trend-panel"><div class="section-heading"><h2>完成趋势</h2></div>
    <div v-if="summary.total" class="trend-chart"><div class="chart-grid"><span>{{ maxBucket }}</span><span>{{ Math.round(maxBucket / 2) }}</span><span>0</span></div><div class="trend-bars" :class="kind"><button v-for="(item, index) in buckets" :key="item.key" :title="`${item.key}：计划 ${item.planned} 项，完成 ${item.completed} 项${item.actualMin ? `，耗时 ${formatMinutes(item.actualMin)}` : ''}`" :aria-label="`${item.key}：计划 ${item.planned} 项，完成 ${item.completed} 项`" :disabled="kind === 'year'" @click="visit(item.key)"><div class="trend-pair"><i class="planned" :style="{ height: `${item.planned / maxBucket * 100}%` }" /><i class="done" :style="{ height: `${item.completed / maxBucket * 100}%` }" /></div><span>{{ kind === 'month' ? (index === 0 || (index + 1) % 5 === 0 ? item.label : '') : item.label }}</span></button></div></div>
    <p v-else class="empty-chart">这段时间没有待办</p>
    <div v-if="showTimeTrend" class="time-strip"><span class="subtle">耗时分布</span><div class="time-bars"><i v-for="item in buckets" :key="item.key" :title="`${item.key}：${item.actualMin ? formatMinutes(item.actualMin) : '无耗时'}`" :style="{ height: `${Math.max(item.actualMin ? 8 : 2, item.actualMin / maxBucketTime * 100)}%` }" :class="{ empty: !item.actualMin }" /></div></div>
  </section>
  <div class="insight-lower">
    <section class="panel"><div class="section-heading"><h2>项目分布</h2><span v-if="byTime" class="subtle">按实际耗时</span></div>
      <div v-if="shares.length" class="tag-chart project-chart"><div v-for="item in shares.slice(0, 10)" :key="item.project"><span :title="item.project">{{ item.project }}</span><div class="tag-track"><i :style="{ width: `${(byTime ? item.actualMin : item.total) / maxShare * 100}%` }" /></div><strong>{{ byTime ? (item.actualMin ? formatMinutes(item.actualMin) : '—') : item.total }}</strong><small>{{ item.completed }}/{{ item.total }}</small></div><p v-if="shares.length > 10" class="subtle">另有 {{ shares.length - 10 }} 个项目</p></div>
      <p v-else class="empty-chart">暂无待办</p>
    </section>
    <section class="panel backlog-panel"><div class="section-heading"><h2>积压待办</h2></div>
      <ul v-if="overdue.length" class="backlog-list"><li v-for="item in overdue.slice(0, 8)" :key="item.goal.id"><button @click="visit(item.goal.date)"><span class="backlog-title">{{ item.goal.title }}</span><span class="backlog-age" :class="{ stale: item.ageDays >= 7 }">{{ ageText(item.ageDays) }}<template v-if="item.goal.carryCount"> · 顺延 {{ item.goal.carryCount }} 次</template></span></button></li></ul>
      <p v-else class="empty-chart">没有积压，保持得很好</p>
      <p v-if="overdue.length > 8" class="subtle">另有 {{ overdue.length - 8 }} 项，可在对应日期查看</p>
    </section>
  </div>
  <section class="panel heat-panel"><div class="section-heading"><h2>记录热力图</h2><span class="subtle">每天的待办与手记数量</span></div>
    <div class="heat-scroll"><div v-if="kind !== 'year'" class="heat-weekdays"><span v-for="name in ['一', '二', '三', '四', '五', '六', '日']" :key="name">{{ name }}</span></div><div class="heat-grid" :class="kind === 'year' ? 'year' : 'cal-view'" :style="heatStyle"><button v-for="(day, index) in days" :key="day.date" :class="`level-${day.level}`" :style="index === 0 ? (kind === 'year' ? { gridRowStart: heatOffset + 1 } : { gridColumnStart: heatOffset + 1 }) : undefined" :title="`${day.date}：${day.count} 条`" :aria-label="`${day.date}：${day.count} 条记录，查看当日`" @click="visit(day.date)"><span v-if="kind !== 'year'">{{ Number(day.date.slice(8)) }}</span></button></div></div>
  </section>
  <div class="insight-lower">
    <section class="panel"><div class="section-heading"><h2>手记标签</h2></div><div v-if="tags.length" class="tag-chart"><div v-for="[tag, count] in tags.slice(0, 8)" :key="tag"><span>{{ tag }}</span><div class="tag-track"><i :style="{ width: `${count / maxTags * 100}%` }" /></div><strong>{{ count }}</strong></div><p v-if="tags.length > 8" class="subtle">另有 {{ tags.length - 8 }} 个标签</p></div><p v-else class="empty-chart">暂无手记</p></section>
    <section class="panel note-distribution"><div class="section-heading"><h2>手记分类</h2></div><div v-for="noteKind in (['progress', 'note', 'blocker'] as const)" :key="noteKind" class="kind-summary"><i :class="noteKind" /><span>{{ kindNames[noteKind] }}</span><strong>{{ notes.filter(note => note.kind === noteKind).length }}</strong><span class="subtle">条</span></div>
      <button v-if="kind !== 'year'" class="insight-report-link" @click="generate"><Sparkles :size="16" />生成这{{ kind === 'week' ? '周' : '个月' }}的报告<ArrowUpRight :size="17" /></button>
      <button class="insight-report-link" :disabled="!goals.length" @click="exportCsv"><Download :size="16" />导出待办明细 CSV<ArrowUpRight :size="17" /></button>
    </section>
  </div>
  </div>
</template>
