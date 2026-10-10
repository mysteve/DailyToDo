<script lang="ts">
const dialogStack: HTMLDivElement[] = []
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { X } from '@lucide/vue'
import { modalOpen } from '../store'
defineProps<{ title: string; wide?: boolean; image?: boolean; editor?: boolean }>()
const emit = defineEmits<{ close: [] }>()
const dialog = ref<HTMLDivElement>()
const previousFocus = document.activeElement as HTMLElement | null
const appElement = document.getElementById('app')
function keydown(event: KeyboardEvent) {
  if (dialog.value !== dialogStack.at(-1)) return
  if (event.key === 'Escape') emit('close')
  if (event.key !== 'Tab' || !dialog.value) return
  const elements = [...dialog.value.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), textarea:not(:disabled), select:not(:disabled), summary, [tabindex="0"]')].filter(element => element.offsetParent !== null)
  const first = elements[0]
  const last = elements[elements.length - 1]
  if (!first || !last) return
  if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus() }
  if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus() }
}
onMounted(async () => {
  if (dialog.value) dialogStack.push(dialog.value)
  modalOpen.value = dialogStack.length > 0
  if (appElement) appElement.inert = true
  document.addEventListener('keydown', keydown)
  await nextTick()
  const target = dialog.value?.querySelector<HTMLElement>('textarea, input:not([type="checkbox"])') || dialog.value?.querySelector<HTMLElement>('button')
  target?.focus()
})
onBeforeUnmount(() => {
  document.removeEventListener('keydown', keydown)
  const index = dialog.value ? dialogStack.indexOf(dialog.value) : -1
  if (index >= 0) dialogStack.splice(index, 1)
  modalOpen.value = dialogStack.length > 0
  if (appElement) appElement.inert = dialogStack.length > 0
  previousFocus?.focus()
})
</script>

<template>
  <Teleport to="body">
    <div class="modal-backdrop" @mousedown.self="emit('close')">
      <div ref="dialog" class="modal" :class="{ wide, 'image-preview': image, 'note-editor-dialog': editor }" role="dialog" aria-modal="true" :aria-label="title">
        <header class="modal-heading"><h2>{{ title }}</h2><button class="icon-button" aria-label="关闭弹窗" @click="emit('close')"><X :size="19" /></button></header>
        <slot />
      </div>
    </div>
  </Teleport>
</template>
