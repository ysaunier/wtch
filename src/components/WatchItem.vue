<script setup lang="ts">
import { ref, computed } from "vue";
import type { WatchState, WatchDetail, WatchStatus } from "../types";
import StatusStyle from "./styles/StatusStyle.vue";
import ProgressStyle from "./styles/ProgressStyle.vue";
import TextStyle from "./styles/TextStyle.vue";

const props = defineProps<{
  watch: WatchState;
}>();

const expanded = ref(props.watch.expand);

const inlineText = computed<string | undefined>(() => {
  const style = props.watch.display.style;
  if (style === "status" && props.watch.display.description) return props.watch.display.description;
  if (style === "text" && props.watch.display.content) return props.watch.display.content;
  if (props.watch.details.length > 0) return "Details";
  return undefined;
});

function toggleExpand(): void {
  if (props.watch.details.length > 0) {
    expanded.value = !expanded.value;
  }
}

function openUrl(): void {
  if (!props.watch.url) return;
  const url = props.watch.url;

  // Use Tauri shell plugin when available, fall back to window.open.
  if (window.__TAURI_INTERNALS__) {
    import("@tauri-apps/plugin-shell")
      .then(({ open }) => open(url))
      .catch(() => window.open(url, "_blank"));
  } else {
    window.open(url, "_blank");
  }
}

function detailStatusColor(detail: WatchDetail): WatchStatus {
  // If detail has a status value (from dynamic details), use it
  if (typeof detail.value === "string" && ["success", "warning", "error", "maintenance", "unknown"].includes(detail.value)) {
    return detail.value as WatchStatus;
  }
  return props.watch.status;
}
</script>

<template>
  <div class="watch-item">
    <div class="watch-header">
      <span class="status-indicator" :class="`dot--${watch.status}`" aria-hidden="true" />

      <button
        v-if="watch.url"
        class="watch-name watch-name--link"
        type="button"
        :title="watch.url"
        @click="openUrl"
      >
        {{ watch.name }}
      </button>
      <span v-else class="watch-name">{{ watch.name }}</span>

      <button
        v-if="watch.details.length > 0"
        class="inline-expand"
        type="button"
        :aria-expanded="expanded"
        @click="toggleExpand"
      >
        <span v-if="inlineText" class="inline-value">{{ inlineText }}</span>
        <svg class="chevron" :class="{ 'chevron--open': expanded }" width="12" height="12" viewBox="0 0 12 12" fill="none" aria-hidden="true">
          <path d="M3 4.5L6 7.5L9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
        </svg>
      </button>
      <span v-else-if="inlineText" class="inline-value">{{ inlineText }}</span>
    </div>

    <div v-if="watch.display.style === 'progress'" class="watch-display">
      <ProgressStyle
        :status="watch.status"
        :title="watch.display.title"
        :description="watch.display.description"
        :value="watch.display.value"
        :min="watch.display.min"
        :max="watch.display.max"
      />
    </div>

    <div v-if="expanded && watch.details.length > 0" class="watch-details">
      <div v-for="(detail, idx) in watch.details" :key="idx" class="detail-row">
        <ProgressStyle
          v-if="detail.style === 'progress'"
          :status="detailStatusColor(detail)"
          :title="detail.title"
          :description="detail.description"
          :value="detail.value"
          :min="detail.min"
          :max="detail.max"
        />
        <TextStyle
          v-else-if="detail.style === 'text'"
          :title="detail.title"
          :content="typeof detail.value === 'string' ? detail.value : String(detail.value ?? '')"
        />
        <div v-else-if="detail.style === 'value'" class="detail-value-row">
          <span class="detail-value-label">{{ detail.title }}</span>
          <span class="detail-value-num">{{ detail.value }}</span>
        </div>
        <StatusStyle
          v-else
          :status="detailStatusColor(detail)"
          :title="detail.title"
          :description="detail.description"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.watch-item {
  padding: 8px 12px;
  border-bottom: 1px solid var(--border);
}

.watch-item:last-child {
  border-bottom: none;
}

.watch-header {
  display: flex;
  align-items: center;
  gap: 7px;
}

.status-indicator {
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

.watch-name {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: var(--text);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.watch-name--link {
  background: none;
  border: none;
  padding: 0;
  cursor: pointer;
  text-align: left;
  text-decoration: none;
}

.watch-name--link:hover {
  color: var(--accent);
  text-decoration: underline;
}

.watch-name--link:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 2px;
  border-radius: 2px;
}

.inline-expand {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: auto;
  background: none;
  border: none;
  padding: 2px 0;
  cursor: pointer;
  color: var(--text-secondary);
  max-width: 55%;
  border-radius: 3px;
}

.inline-expand:hover {
  color: var(--text);
}

.inline-expand:focus-visible {
  outline: 1px solid var(--accent);
  outline-offset: 2px;
}

.chevron {
  transition: transform 0.2s ease;
}

.chevron--open {
  transform: rotate(180deg);
}

.watch-display {
  padding-left: 15px;
  margin-top: 4px;
}

.watch-details {
  margin-top: 6px;
  margin-left: 5px;
  padding-left: 10px;
  display: flex;
  flex-direction: column;
  gap: 5px;
  border-left: 2px solid var(--border);
}

.detail-row {
  padding: 1px 0;
}

.detail-value-row {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  padding: 2px 0;
}

.detail-value-label {
  font-size: 12px;
  color: var(--text-secondary);
}

.detail-value-num {
  font-size: 13px;
  color: var(--text);
  font-weight: 500;
}

.inline-value {
  font-size: 11px;
  color: inherit;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: right;
}
</style>
