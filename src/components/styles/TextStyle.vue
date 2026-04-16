<script setup lang="ts">
defineProps<{
  title?: string;
  content?: string;
}>();
</script>

<template>
  <div class="text-style">
    <span v-if="title" class="text-title">{{ title }}</span>
    <span v-if="content" class="text-content">{{ content }}</span>
  </div>
</template>

<style scoped>
.text-style {
  display: flex;
  align-items: baseline;
  gap: 8px;
  padding: 2px 0;
}

.text-title {
  font-size: 12px;
  color: var(--text-secondary);
  white-space: nowrap;
  flex-shrink: 0;
}

.text-content {
  font-size: 13px;
  color: var(--text);
  font-family: "Consolas", "Courier New", monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
