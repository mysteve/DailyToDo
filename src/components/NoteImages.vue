<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { ChevronLeft, ChevronRight, ImageOff, LoaderCircle, X } from '@lucide/vue'
import { api } from '../api'
import type { NoteImage } from '../domain'
import ModalShell from './ModalShell.vue'

const props = defineProps<{ images: NoteImage[]; urls?: Record<string, string>; editable?: boolean; disabled?: boolean }>()
const emit = defineEmits<{ remove: [id: string] }>()
const loaded = ref<Record<string, string>>({})
const failed = ref<Record<string, boolean>>({})
const selectedId = ref('')
const selectedIndex = computed(() => props.images.findIndex(image => image.id === selectedId.value))
const selected = computed(() => props.images[selectedIndex.value])
const selectedUrl = computed(() => selected.value && url(selected.value.id))
function url(id: string) { return props.urls?.[id] || loaded.value[id] }
watch(() => props.images, async images => {
  await Promise.all(images.map(async image => {
    if (url(image.id)) return
    try { loaded.value[image.id] = await api.readImage(image) }
    catch { failed.value[image.id] = true }
  }))
}, { immediate: true })
function next(amount: number) {
  const image = props.images[(selectedIndex.value + amount + props.images.length) % props.images.length]
  if (image) selectedId.value = image.id
}
</script>

<template>
  <div class="note-images" :class="{ editable }">
    <div v-for="(image, index) in images" :key="image.id" class="note-image-tile">
      <button type="button" class="note-image-open" :aria-label="`查看图片 ${index + 1}：${image.name}`" :disabled="!url(image.id)" @click="selectedId = image.id">
        <img v-if="url(image.id)" :src="url(image.id)" :alt="image.name" loading="lazy" />
        <span v-else-if="failed[image.id]" class="image-placeholder"><ImageOff :size="20" />图片无法读取</span>
        <LoaderCircle v-else class="spin" :size="20" />
      </button>
      <button v-if="editable" type="button" class="note-image-remove" :aria-label="`移除图片 ${index + 1}`" :disabled="disabled" @click="emit('remove', image.id)"><X :size="13" /></button>
    </div>
  </div>
  <ModalShell v-if="selected" title="图片预览" image @close="selectedId = ''">
    <div class="image-preview-body" @keydown.left.prevent="next(-1)" @keydown.right.prevent="next(1)">
      <img v-if="selectedUrl" :src="selectedUrl" :alt="selected.name" />
      <div v-if="images.length > 1" class="image-preview-controls"><button class="icon-button" aria-label="上一张图片" @click="next(-1)"><ChevronLeft :size="20" /></button><span>{{ selectedIndex + 1 }} / {{ images.length }}</span><button class="icon-button" aria-label="下一张图片" @click="next(1)"><ChevronRight :size="20" /></button></div>
    </div>
  </ModalShell>
</template>
