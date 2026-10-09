<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { Copy, Minus, Square, X } from '@lucide/vue'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { UnlistenFn } from '@tauri-apps/api/event'
import { notify } from '../store'

const appWindow = getCurrentWindow()
const maximized = ref(false)
const pending = ref(false)
let unlisten: UnlistenFn | undefined
let disposed = false

async function refreshState() {
  const value = await appWindow.isMaximized()
  if (!disposed) maximized.value = value
}

async function control(action: 'minimize' | 'maximize' | 'close') {
  if (pending.value) return
  pending.value = true
  try {
    if (action === 'minimize') await appWindow.minimize()
    else if (action === 'close') await appWindow.close()
    else { await appWindow.toggleMaximize(); await refreshState() }
  } catch (error) {
    notify(`窗口操作失败：${error instanceof Error ? error.message : String(error)}`)
  } finally { pending.value = false }
}

onMounted(async () => {
  try {
    await refreshState()
    const stop = await appWindow.onResized(() => { void refreshState().catch(() => {}) })
    if (disposed) stop()
    else unlisten = stop
  } catch (error) { notify(`窗口状态读取失败：${String(error)}`) }
})
onBeforeUnmount(() => { disposed = true; unlisten?.() })
</script>

<template>
  <div class="window-controls-space" aria-hidden="true" />
  <Teleport to="body"><div class="window-controls" role="group" aria-label="窗口控制">
    <button class="window-button" aria-label="最小化" title="最小化" :disabled="pending" @click="control('minimize')"><Minus :size="16" :stroke-width="1.7" /></button>
    <button class="window-button" :aria-label="maximized ? '还原' : '最大化'" :title="maximized ? '还原' : '最大化'" :disabled="pending" @click="control('maximize')"><Copy v-if="maximized" :size="14" :stroke-width="1.7" /><Square v-else :size="14" :stroke-width="1.7" /></button>
    <button class="window-button window-close" aria-label="关闭窗口" title="关闭窗口" :disabled="pending" @click="control('close')"><X :size="17" :stroke-width="1.7" /></button>
  </div></Teleport>
</template>
