<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronLeft, ChevronRight } from '@lucide/vue'
import { addDays, dateKey, parseDate } from '../domain'
import { selectedDate, workspace } from '../store'

const month = ref(selectedDate.value.slice(0, 7))
watch(selectedDate, date => { month.value = date.slice(0, 7) })
const label = computed(() => `${Number(month.value.slice(0, 4))}年 ${Number(month.value.slice(5))}月`)
const days = computed(() => {
  const first = `${month.value}-01`
  const firstDate = parseDate(first)
  const start = addDays(first, -((firstDate.getDay() + 6) % 7))
  const numberOfDays = new Date(firstDate.getFullYear(), firstDate.getMonth() + 1, 0).getDate()
  const count = Math.ceil((((firstDate.getDay() + 6) % 7) + numberOfDays) / 7) * 7
  return Array.from({ length: count }, (_, i) => addDays(start, i))
})
const recorded = computed(() => new Set([...workspace.value.notes.map(note => note.date), ...workspace.value.goals.map(goal => goal.date)]))
const today = dateKey(new Date())
function shift(amount: number) {
  const date = parseDate(`${month.value}-01`)
  date.setMonth(date.getMonth() + amount)
  month.value = dateKey(date).slice(0, 7)
}
</script>

<template>
  <section class="calendar" aria-label="记录日历">
    <div class="calendar-heading">
      <span>{{ label }}</span>
      <div>
        <button class="icon-button small" aria-label="上个月" @click="shift(-1)"><ChevronLeft :size="15" /></button>
        <button class="icon-button small" aria-label="下个月" @click="shift(1)"><ChevronRight :size="15" /></button>
      </div>
    </div>
    <div class="calendar-grid weekdays"><span v-for="day in ['一', '二', '三', '四', '五', '六', '日']" :key="day">{{ day }}</span></div>
    <div class="calendar-grid">
      <button v-for="day in days" :key="day" class="calendar-day" :class="{ selected: day === selectedDate, outside: !day.startsWith(month), today: day === today, recorded: recorded.has(day) }" :aria-label="`${day}${recorded.has(day) ? '，有记录' : ''}`" :aria-pressed="day === selectedDate" @click="selectedDate = day">{{ Number(day.slice(8)) }}<i v-if="recorded.has(day)" /></button>
    </div>
  </section>
</template>
