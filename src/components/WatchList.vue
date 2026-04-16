<script setup lang="ts">
import { ref, computed } from "vue";
import type { AppState, WatchState } from "../types";
import WatchItem from "./WatchItem.vue";
import SettingsPanel from "./SettingsPanel.vue";

type GroupedEntry = { type: "group"; name: string } | { type: "watch"; watch: WatchState };

const props = defineProps<{
  state: AppState;
  timeSinceLastCheck: string;
}>();

const groupedWatches = computed<GroupedEntry[]>(() => {
  const groups = props.state.groups;
  if (!groups || Object.keys(groups).length === 0) {
    return props.state.watches.map((w) => ({ type: "watch" as const, watch: w }));
  }

  const entries: GroupedEntry[] = [];
  const placed = new Set<string>();

  for (const [groupName, watchNames] of Object.entries(groups)) {
    const members = watchNames
      .map((n) => props.state.watches.find((w) => w.name === n))
      .filter((w): w is WatchState => w !== undefined);
    if (members.length === 0) continue;
    entries.push({ type: "group", name: groupName });
    for (const w of members) {
      entries.push({ type: "watch", watch: w });
      placed.add(w.name);
    }
  }

  for (const w of props.state.watches) {
    if (!placed.has(w.name)) {
      entries.push({ type: "watch", watch: w });
    }
  }

  return entries;
});

const emit = defineEmits<{
  refresh: [];
}>();

const showSettings = ref(false);

function onRefresh(): void {
  emit("refresh");
}

function onToggleSettings(): void {
  showSettings.value = !showSettings.value;
}

function onSettingsReload(): void {
  emit("refresh");
}

async function onMinimize(): Promise<void> {
  if (window.__TAURI_INTERNALS__) {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    getCurrentWindow().hide();
  }
}

async function onQuit(): Promise<void> {
  if (window.__TAURI_INTERNALS__) {
    const { invoke } = await import("@tauri-apps/api/core");
    invoke("plugin:process|exit", { code: 0 });
  }
}
</script>

<template>
  <div class="watch-list">
    <header class="list-header" data-tauri-drag-region>
      <span class="header-title" data-tauri-drag-region>wtch</span>
      <span class="header-ago" data-tauri-drag-region>{{ timeSinceLastCheck }}</span>
      <div class="header-actions">
        <button
          class="header-btn"
          :class="{ 'header-btn--active': showSettings }"
          type="button"
          title="Settings"
          aria-label="Settings"
          @click="onToggleSettings"
        >
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-4 0v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83-2.83l.06-.06A1.65 1.65 0 004.68 15a1.65 1.65 0 00-1.51-1H3a2 2 0 010-4h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 012.83-2.83l.06.06A1.65 1.65 0 009 4.68a1.65 1.65 0 001-1.51V3a2 2 0 014 0v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 2.83l-.06.06A1.65 1.65 0 0019.4 9a1.65 1.65 0 001.51 1H21a2 2 0 010 4h-.09a1.65 1.65 0 00-1.51 1z" />
          </svg>
        </button>
        <button
          class="header-btn"
          type="button"
          title="Refresh now"
          aria-label="Refresh now"
          @click="onRefresh"
        >
          <svg width="11" height="11" viewBox="0 0 13 13" fill="none" aria-hidden="true">
            <path
              d="M11.5 6.5A5 5 0 1 1 6.5 1.5a5 5 0 0 1 3.536 1.464L11.5 4.5"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
            />
            <path
              d="M9 4.5h2.5V2"
              stroke="currentColor"
              stroke-width="1.5"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </button>
        <button
          class="header-btn"
          type="button"
          title="Hide to tray"
          aria-label="Hide to tray"
          @click="onMinimize"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
            <path d="M1 5h8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
        <button
          class="header-btn header-btn-close"
          type="button"
          title="Quit wtch"
          aria-label="Quit wtch"
          @click="onQuit"
        >
          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
            <path d="M1 1l8 8M9 1l-8 8" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" />
          </svg>
        </button>
      </div>
    </header>

    <Transition name="panel" mode="out-in">
      <SettingsPanel
        v-if="showSettings"
        key="settings"
        :state="state"
        @reload="onSettingsReload"
      />

      <div v-else key="watches" class="watches-body">
        <div v-if="state.watches.length === 0" class="empty-state">
          No watches configured.
        </div>

        <template v-for="(entry, idx) in groupedWatches" :key="idx">
          <div v-if="entry.type === 'group'" class="group-header">
            {{ entry.name }}
          </div>
          <WatchItem v-else :watch="entry.watch" />
        </template>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.watch-list {
  display: flex;
  flex-direction: column;
  min-height: 0;
  flex: 1;
}

.list-header {
  display: flex;
  align-items: center;
  padding: 6px 4px 6px 12px;
  border-bottom: 1px solid var(--border);
  cursor: grab;
  user-select: none;
}

.list-header:active {
  cursor: grabbing;
}

.header-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: 0.5px;
}

.header-ago {
  flex: 1;
  font-size: 11px;
  color: var(--text-secondary);
  opacity: 0.6;
  text-align: right;
  padding-right: 6px;
}

.header-actions {
  display: flex;
  gap: 1px;
}

.header-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 22px;
  background: none;
  border: none;
  border-radius: 4px;
  color: var(--text-secondary);
  cursor: pointer;
}

.header-btn:hover {
  background: var(--border);
  color: var(--text);
}

.header-btn--active {
  background: var(--border);
  color: var(--text);
}

.header-btn-close:hover {
  background: var(--error);
  color: white;
}

.watches-body {
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
}

.watches-body::-webkit-scrollbar {
  width: 6px;
}

.watches-body::-webkit-scrollbar-track {
  background: transparent;
}

.watches-body::-webkit-scrollbar-thumb {
  background: var(--border);
  border-radius: 3px;
}

.watches-body::-webkit-scrollbar-thumb:hover {
  background: var(--text-secondary);
}

.group-header {
  padding: 5px 12px 3px;
  font-size: 10px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.8px;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border);
  background: var(--surface);
}

.empty-state {
  padding: 16px 12px;
  font-size: 13px;
  color: var(--text-secondary);
  text-align: center;
}

.panel-enter-active,
.panel-leave-active {
  transition: opacity 0.15s ease;
}

.panel-enter-from,
.panel-leave-to {
  opacity: 0;
}
</style>
