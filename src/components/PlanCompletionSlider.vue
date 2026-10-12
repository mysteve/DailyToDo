<script setup lang="ts">
defineProps<{ modelValue: number; label: string; disabled?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: number]; change: [value: number] }>()
</script>

<template>
  <label class="plan-completion">
    <span class="plan-completion-label"><span>完成度</span><output>{{ modelValue }}%</output></span>
    <input type="range" min="0" max="100" step="1" :value="modelValue" :aria-label="label" :aria-valuetext="`${modelValue}%`" :disabled="disabled" @input="emit('update:modelValue', Number(($event.target as HTMLInputElement).value))" @change="emit('change', Number(($event.target as HTMLInputElement).value))" />
  </label>
</template>
