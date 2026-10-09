<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from 'vue'
import { CalendarDays, ChevronDown, ChevronLeft, ChevronRight } from '@lucide/vue'
import { addDays, dateKey, parseDate, periodLabel, shortDate, type DateRange } from '../domain'

const props = defineProps<{ modelValue: DateRange; compact?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: DateRange]; openChange: [open: boolean] }>()
const pickerId = useId()
const root = ref<HTMLDivElement>()
const panel = ref<HTMLDivElement>()
const trigger = ref<HTMLButtonElement>()
const open = ref(false)
const month = ref(props.modelValue.start.slice(0, 7))
const draftStart = ref(props.modelValue.start)
const draftEnd = ref(props.modelValue.end)
const selectionStart = ref('')
const hoverDate = ref('')
const minimum = '1900-01-01'
const maximum = '9999-12-31'
let dragOrigin = ''
let dragPointer = -1
let dragMoved = false
let ignoreClick = false
let clickResetTimer: ReturnType<typeof setTimeout> | undefined
let beforeDrag = { start: '', end: '', selection: '' }

const label = computed(() => {
  const { start, end } = props.modelValue
  if (!props.compact) return periodLabel(start, end)
  const includeYear = start.slice(0, 4) !== end.slice(0, 4) || start.slice(0, 4) !== String(new Date().getFullYear())
  return start === end ? shortDate(start, includeYear) : `${shortDate(start, includeYear)} — ${shortDate(end, includeYear)}`
})
const monthLabel = computed(() => `${Number(month.value.slice(0, 4))}年 ${Number(month.value.slice(5))}月`)
const days = computed(() => {
  const first = `${month.value}-01`
  const date = parseDate(first)
  const offset = (date.getDay() + 6) % 7
  const count = Math.ceil((offset + new Date(date.getFullYear(), date.getMonth() + 1, 0).getDate()) / 7) * 7
  const start = addDays(first, -offset)
  return Array.from({ length: count }, (_, index) => addDays(start, index))
})
const rangeError = computed(() => {
  if (!draftStart.value || !draftEnd.value) return '请选择开始日期和结束日期'
  try { parseDate(draftStart.value); parseDate(draftEnd.value) } catch { return '请选择有效日期' }
  if (!supported(draftStart.value) || !supported(draftEnd.value)) return '日期必须在 1900 至 9999 年之间'
  return draftStart.value > draftEnd.value ? '结束日期不能早于开始日期' : ''
})
const preview = computed(() => selectionStart.value && hoverDate.value
  ? ordered(selectionStart.value, hoverDate.value)
  : { start: draftStart.value, end: draftEnd.value || draftStart.value })

function supported(day: string) { return day >= minimum && day <= maximum }
function ordered(first: string, second: string): DateRange { return first <= second ? { start: first, end: second } : { start: second, end: first } }
function close(returnFocus = true) {
  open.value = false
  dragOrigin = ''
  emit('openChange', false)
  if (returnFocus) void nextTick(() => trigger.value?.focus())
}
async function toggle() {
  if (open.value) return close()
  draftStart.value = props.modelValue.start
  draftEnd.value = props.modelValue.end
  month.value = props.modelValue.start.slice(0, 7)
  selectionStart.value = ''
  hoverDate.value = ''
  open.value = true
  emit('openChange', true)
  await nextTick()
  panel.value?.querySelector<HTMLButtonElement>(`[data-range-day="${draftStart.value}"]`)?.focus()
}
function shiftMonth(amount: number) {
  const date = parseDate(`${month.value}-01`)
  date.setMonth(date.getMonth() + amount)
  const next = dateKey(date).slice(0, 7)
  if (next >= '1900-01' && next <= '9999-12') month.value = next
}
function chooseDay(day: string) {
  if (ignoreClick) return
  if (selectionStart.value) {
    const range = ordered(selectionStart.value, day)
    draftStart.value = range.start
    draftEnd.value = range.end
    selectionStart.value = ''
  } else {
    draftStart.value = day
    draftEnd.value = ''
    selectionStart.value = day
  }
  hoverDate.value = ''
}
function hoverDay(day: string) { if (selectionStart.value) hoverDate.value = day }
function fieldChanged() {
  selectionStart.value = ''
  hoverDate.value = ''
  try { if (supported(draftStart.value)) { parseDate(draftStart.value); month.value = draftStart.value.slice(0, 7) } } catch { }
}
function apply() {
  if (rangeError.value) return
  emit('update:modelValue', { start: draftStart.value, end: draftEnd.value })
  close()
}
function beginDrag(day: string, event: PointerEvent) {
  if (event.button !== 0) return
  dragOrigin = day
  dragPointer = event.pointerId
  dragMoved = false
  beforeDrag = { start: draftStart.value, end: draftEnd.value, selection: selectionStart.value }
  const cell = event.currentTarget as HTMLButtonElement
  cell.focus()
}
function pointerMove(event: PointerEvent) {
  if (!dragOrigin || event.pointerId !== dragPointer) return
  const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLButtonElement>('[data-range-day]')
  if (!target || !panel.value?.contains(target) || target.disabled) return
  const day = target.dataset.rangeDay!
  if (day === dragOrigin && !dragMoved) return
  dragMoved = true
  const range = ordered(dragOrigin, day)
  draftStart.value = range.start
  draftEnd.value = range.end
  selectionStart.value = ''
  hoverDate.value = ''
  event.preventDefault()
}
function pointerUp(event: PointerEvent) {
  if (!dragOrigin || event.pointerId !== dragPointer) return
  if (dragMoved) {
    ignoreClick = true
    clearTimeout(clickResetTimer)
    clickResetTimer = setTimeout(() => { ignoreClick = false }, 0)
  }
  dragOrigin = ''
}
function pointerCancel() {
  if (dragOrigin && dragMoved) {
    draftStart.value = beforeDrag.start
    draftEnd.value = beforeDrag.end
    selectionStart.value = beforeDrag.selection
  }
  dragOrigin = ''
}
function outsideClick(event: PointerEvent) {
  if (event.target instanceof Element && event.target.closest('button:disabled')) return
  if (open.value && event.target instanceof Node && !root.value?.contains(event.target)) close(false)
}
async function moveFocus(day: string, event: KeyboardEvent) {
  const offsets: Record<string, number> = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }
  const amount = offsets[event.key]
  if (amount === undefined) return
  event.preventDefault()
  const next = addDays(day, amount)
  if (!supported(next)) return
  month.value = next.slice(0, 7)
  hoverDay(next)
  await nextTick()
  panel.value?.querySelector<HTMLButtonElement>(`[data-range-day="${next}"]`)?.focus()
}
onMounted(() => {
  document.addEventListener('pointerdown', outsideClick)
  document.addEventListener('pointermove', pointerMove, { passive: false })
  document.addEventListener('pointerup', pointerUp)
  document.addEventListener('pointercancel', pointerCancel)
})
onBeforeUnmount(() => {
  clearTimeout(clickResetTimer)
  document.removeEventListener('pointerdown', outsideClick)
  document.removeEventListener('pointermove', pointerMove)
  document.removeEventListener('pointerup', pointerUp)
  document.removeEventListener('pointercancel', pointerCancel)
  emit('openChange', false)
})
</script>

<template>
  <div ref="root" class="range-picker" @keydown.esc.stop.prevent="close()">
    <button ref="trigger" type="button" class="range-picker-trigger" :aria-expanded="open" :aria-controls="`${pickerId}-panel`" aria-label="选择报告时间范围" @click="toggle"><CalendarDays :size="15" /><span>{{ label }}</span><ChevronDown :size="13" :class="{ rotated: open }" /></button>
    <div v-if="open" :id="`${pickerId}-panel`" ref="panel" class="range-picker-panel" role="group" aria-label="报告时间范围">
      <div class="range-date-fields"><label :for="`${pickerId}-start`">开始日期<input :id="`${pickerId}-start`" v-model="draftStart" type="date" :min="minimum" :max="maximum" required @change="fieldChanged" /></label><label :for="`${pickerId}-end`">结束日期<input :id="`${pickerId}-end`" v-model="draftEnd" type="date" :min="minimum" :max="maximum" required @change="fieldChanged" /></label></div>
      <div class="range-month-heading"><button type="button" class="icon-button small" aria-label="范围日历上个月" :disabled="month === '1900-01'" @click="shiftMonth(-1)"><ChevronLeft :size="15" /></button><strong>{{ monthLabel }}</strong><button type="button" class="icon-button small" aria-label="范围日历下个月" :disabled="month === '9999-12'" @click="shiftMonth(1)"><ChevronRight :size="15" /></button></div>
      <div class="range-weekdays"><span v-for="day in ['一', '二', '三', '四', '五', '六', '日']" :key="day">{{ day }}</span></div>
      <div class="range-calendar-grid"><button v-for="day in days" :key="day" type="button" class="range-calendar-day" :class="{ outside: !day.startsWith(month), 'in-range': day >= preview.start && day <= preview.end, 'range-start': day === preview.start, 'range-end': day === preview.end }" :data-range-day="day" :disabled="!supported(day)" :aria-label="day" :aria-pressed="day >= preview.start && day <= preview.end" @pointerdown="beginDrag(day, $event)" @mouseenter="hoverDay(day)" @focus="hoverDay(day)" @keydown="moveFocus(day, $event)" @click="chooseDay(day)">{{ Number(day.slice(-2)) }}</button></div>
      <p v-if="rangeError && draftStart && draftEnd" class="range-error" role="alert">{{ rangeError }}</p>
      <div class="range-picker-actions"><button type="button" class="text-button" @click="close()">取消</button><button type="button" class="button primary" :disabled="Boolean(rangeError)" @click="apply">确定</button></div>
    </div>
  </div>
</template>
