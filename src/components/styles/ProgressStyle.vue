<script setup lang="ts">
import type { WatchStatus } from "../../types";

const props = defineProps<{
  status: WatchStatus;
  title?: string;
  description?: string;
  value?: string | number;
  min?: number;
  max?: number;
}>();

function toNumber(v: string | number | undefined): number {
  if (v === undefined) return 0;
  return typeof v === "number" ? v : parseFloat(v);
}

function percentage(): number {
  const min = props.min ?? 0;
  const max = props.max ?? 100;
  const val = toNumber(props.value);
  if (max <= min) return 0;
  return Math.min(100, Math.max(0, ((val - min) / (max - min)) * 100));
}
</script>

<template>
  <div class="progress-style">
    <div v-if="title || description" class="progress-header">
      <span v-if="title" class="progress-title">{{ title }}</span>
      <span v-if="description" class="progress-desc">{{ description }}</span>
    </div>
    <div class="progress-track" role="progressbar" :aria-valuenow="toNumber(value)" :aria-valuemin="min ?? 0" :aria-valuemax="max ?? 100">
      <div class="progress-fill" :class="`fill--${status}`" :style="{ width: `${percentage()}%` }" />
    </div>
  </div>
</template>

<style scoped>
.progress-style {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 2px 0;
}

.progress-header {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.progress-title {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex-shrink: 1;
}

.progress-desc {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}

.progress-track {
  height: 5px;
  background: var(--border);
  border-radius: 3px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 3px;
  transition: width 0.3s ease;
}

.fill--success { background: var(--success); }
.fill--warning { background: var(--warning); }
.fill--error   { background: var(--error); }
.fill--unknown { background: var(--unknown); }
</style>
