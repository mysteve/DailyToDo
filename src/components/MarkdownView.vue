<script setup lang="ts">
import { computed } from 'vue'
const props = defineProps<{ content: string }>()
// Plain text interpolation keeps both AI output and user records out of HTML execution.
interface Block { kind: string; text?: string; headers?: string[]; rows?: string[][] }
const blocks = computed(() => {
  const lines = props.content.split('\n')
  const result: Block[] = []
  const cells = (line: string) => line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map(cell => cell.trim())
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!
    if (line.trim().startsWith('|') && /^\s*\|?\s*:?-{3,}/.test(lines[i + 1] || '') && cells(lines[i + 1]!).every(cell => /^:?-{3,}:?$/.test(cell))) {
      const headers = cells(line)
      const rows: string[][] = []
      i += 2
      while (i < lines.length && lines[i]!.trim().startsWith('|')) { rows.push(cells(lines[i]!)); i++ }
      i--
      result.push({ kind: 'table', headers, rows }); continue
    }
    const heading = line.match(/^(#{1,3})\s+(.+)$/)
    if (heading) { result.push({ kind: `h${heading[1]!.length}`, text: heading[2]! }); continue }
    if (/^- \[[x ]\]/.test(line)) { result.push({ kind: line.startsWith('- [x]') ? 'done' : 'pending', text: line.slice(6) }); continue }
    if (/^[-*] /.test(line)) { result.push({ kind: 'li', text: line.slice(2) }); continue }
    result.push({ kind: line.trim() ? 'p' : 'space', text: line.replace(/\*\*(.*?)\*\*/g, '$1') })
  }
  return result
})
</script>

<template>
  <article class="markdown">
    <template v-for="(block, index) in blocks" :key="index">
      <div v-if="block.kind === 'table'" class="markdown-table-wrap" tabindex="0" aria-label="计划进展对照表"><table :class="{ 'completion-comparison-table': block.headers?.includes('完成度（上周 → 本周）') }"><thead><tr><th v-for="(cell, col) in block.headers" :key="col" scope="col">{{ cell }}</th></tr></thead><tbody><tr v-for="(row, ri) in block.rows" :key="ri"><td v-for="(cell, ci) in row" :key="ci">{{ cell }}</td></tr></tbody></table></div>
      <h1 v-else-if="block.kind === 'h1'">{{ block.text }}</h1>
      <h2 v-else-if="block.kind === 'h2'">{{ block.text }}</h2>
      <h3 v-else-if="block.kind === 'h3'">{{ block.text }}</h3>
      <p v-else-if="block.kind === 'li'" class="markdown-list"><span>•</span>{{ block.text }}</p>
      <p v-else-if="block.kind === 'done' || block.kind === 'pending'" class="markdown-list"><span>{{ block.kind === 'done' ? '☑' : '☐' }}</span>{{ block.text }}</p>
      <p v-else-if="block.kind === 'p'">{{ block.text }}</p>
      <div v-else class="markdown-space" />
    </template>
  </article>
</template>
