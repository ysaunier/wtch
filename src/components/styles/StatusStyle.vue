<script setup lang="ts">
import type { WatchStatus } from "../../types";

defineProps<{
  status: WatchStatus;
  title?: string;
  description?: string;
}>();
</script>

<template>
  <div class="status-style">
    <span class="status-dot" :class="`dot--${status}`" aria-hidden="true" />
    <div class="status-text">
      <span v-if="title" class="status-title">{{ title }}</span>
      <span v-if="description" class="status-desc">{{ description }}</span>
    </div>
  </div>
</template>

<style scoped>
.status-style {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 0;
}

.status-dot {
  flex-shrink: 0;
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.dot--success { background: var(--success); }
.dot--warning { background: var(--warning); }
.dot--error   { background: var(--error); }
.dot--maintenance { background: var(--maintenance); }
.dot--unknown { background: var(--unknown); }

.status-text {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.status-title {
  font-size: 13px;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.status-desc {
  font-size: 11px;
  color: var(--text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
