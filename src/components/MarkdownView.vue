<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{ content: string }>()
// Plain text interpolation keeps both AI output and user records out of HTML execution.
const blocks = computed(() => props.content.split('\n').map(line => {
  const heading = line.match(/^(#{1,3})\s+(.+)$/)
  if (heading) return { kind: `h${heading[1]!.length}`, text: heading[2]! }
  if (/^- \[[x ]\]/.test(line)) return { kind: line.startsWith('- [x]') ? 'done' : 'pending', text: line.slice(6) }
  if (/^[-*] /.test(line)) return { kind: 'li', text: line.slice(2) }
  return { kind: line.trim() ? 'p' : 'space', text: line.replace(/\*\*(.*?)\*\*/g, '$1') }
}))
</script>

<template>
  <article class="markdown">
    <template v-for="(block, index) in blocks" :key="index">
      <h1 v-if="block.kind === 'h1'">{{ block.text }}</h1>
      <h2 v-else-if="block.kind === 'h2'">{{ block.text }}</h2>
      <h3 v-else-if="block.kind === 'h3'">{{ block.text }}</h3>
      <p v-else-if="block.kind === 'li'" class="markdown-list"><span>•</span>{{ block.text }}</p>
      <p v-else-if="block.kind === 'done' || block.kind === 'pending'" class="markdown-list"><span>{{ block.kind === 'done' ? '☑' : '☐' }}</span>{{ block.text }}</p>
      <p v-else-if="block.kind === 'p'">{{ block.text }}</p>
      <div v-else class="markdown-space" />
    </template>
  </article>
</template>
