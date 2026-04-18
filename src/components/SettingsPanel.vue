<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import type { AppState, PresetInfo } from "../types";
import { useTheme } from "../composables/useTheme";
import type { Theme } from "../composables/useTheme";

type Tab = "presets" | "order" | "actions" | "about";

const props = defineProps<{
  state: AppState;
}>();

const emit = defineEmits<{
  close: [];
  reload: [];
}>();

const activeTab = ref<Tab>("presets");
const presets = ref<PresetInfo[]>([]);
const presetSearch = ref("");
const loading = ref(false);
const debugMode = ref(false);
const allowScripts = ref(false);
const appVersion = ref("0.0.0");
const { currentTheme, setTheme } = useTheme();
const themeOptions: { value: Theme; label: string }[] = [
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (window.__TAURI_INTERNALS__) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  return null;
}

async function refreshPresets(): Promise<void> {
  const result = await tauriInvoke<PresetInfo[]>("list_presets");
  if (Array.isArray(result)) {
    presets.value = result;
  }
}

async function reloadAndRefresh(): Promise<void> {
  loading.value = true;
  await tauriInvoke("reload_config");
  await refreshPresets();
  emit("reload");
  loading.value = false;
}

onMounted(async () => {
  await refreshPresets();
  const dbg = await tauriInvoke<boolean>("get_debug");
  if (dbg !== null) debugMode.value = dbg;
  const scripts = await tauriInvoke<boolean>("get_allow_scripts");
  if (scripts !== null) allowScripts.value = scripts;
  try {
    if (window.__TAURI_INTERNALS__) {
      const { getVersion } = await import("@tauri-apps/api/app");
      appVersion.value = await getVersion();
    }
  } catch {
    appVersion.value = "0.0.0";
  }
});

// Re-fetch presets when state changes (after reload)
watch(() => props.state.watches.length, refreshPresets);

const filteredPresets = computed(() => {
  const q = presetSearch.value.toLowerCase().trim();
  if (!q) return presets.value;
  return presets.value.filter((p) => p.name.toLowerCase().includes(q));
});

async function onTogglePreset(preset: PresetInfo): Promise<void> {
  const newEnabled = !preset.enabled;
  await tauriInvoke("toggle_preset", { name: preset.name, enabled: newEnabled });
  await reloadAndRefresh();
}

async function onMoveUp(index: number): Promise<void> {
  if (index <= 0) return;
  const order = props.state.watches.map((w) => w.name);
  [order[index - 1], order[index]] = [order[index], order[index - 1]];
  await tauriInvoke("reorder_watches", { order });
  await reloadAndRefresh();
}

async function onMoveDown(index: number): Promise<void> {
  const order = props.state.watches.map((w) => w.name);
  if (index >= order.length - 1) return;
  [order[index], order[index + 1]] = [order[index + 1], order[index]];
  await tauriInvoke("reorder_watches", { order });
  await reloadAndRefresh();
}

async function onOpenConfig(): Promise<void> {
  await tauriInvoke("open_config");
}

async function onOpenConfigDir(): Promise<void> {
  await tauriInvoke("open_config_dir");
}

async function onReloadConfig(): Promise<void> {
  await reloadAndRefresh();
}

async function onToggleDebug(): Promise<void> {
  const newVal = !debugMode.value;
  await tauriInvoke("set_debug", { enabled: newVal });
  debugMode.value = newVal;
}

async function onToggleAllowScripts(): Promise<void> {
  const newVal = !allowScripts.value;
  await tauriInvoke("set_allow_scripts", { enabled: newVal });
  allowScripts.value = newVal;
}

function openUrl(url: string): void {
  if (window.__TAURI_INTERNALS__) {
    import("@tauri-apps/plugin-shell")
      .then(({ open }) => open(url))
      .catch(() => window.open(url, "_blank", "noopener,noreferrer"));
  } else {
    window.open(url, "_blank", "noopener,noreferrer");
  }
}
</script>

<template>
  <div class="settings-panel">
    <nav class="tabs">
      <button
        class="tab"
        :class="{ 'tab--active': activeTab === 'presets' }"
        @click="activeTab = 'presets'"
      >Presets</button>
      <button
        class="tab"
        :class="{ 'tab--active': activeTab === 'order' }"
        @click="activeTab = 'order'"
      >Order</button>
      <button
        class="tab"
        :class="{ 'tab--active': activeTab === 'actions' }"
        @click="activeTab = 'actions'"
      >Config</button>
      <button
        class="tab"
        :class="{ 'tab--active': activeTab === 'about' }"
        @click="activeTab = 'about'"
      >About</button>
    </nav>

    <div class="tab-body">
      <!-- Presets -->
      <div v-if="activeTab === 'presets'" class="tab-content">
        <input
          v-model="presetSearch"
          class="search-input"
          type="text"
          placeholder="Search presets..."
        />
        <div v-if="filteredPresets.length === 0" class="empty-msg">No presets found</div>
        <div
          v-for="preset in filteredPresets"
          :key="preset.name"
          class="row"
        >
          <span class="row-label">{{ preset.name }}</span>
          <button
            class="toggle-btn"
            :class="{ 'toggle-btn--on': preset.enabled, 'toggle-btn--loading': loading }"
            type="button"
            :disabled="loading"
            :aria-pressed="preset.enabled"
            @click="onTogglePreset(preset)"
          >
            <span class="toggle-thumb" />
          </button>
        </div>
      </div>

      <!-- Order -->
      <div v-if="activeTab === 'order'" class="tab-content">
        <div v-if="state.watches.length === 0" class="empty-msg">No watches to reorder</div>
        <div
          v-for="(w, idx) in state.watches"
          :key="w.name"
          class="row"
        >
          <span class="row-label">{{ w.name }}</span>
          <div class="arrow-group">
            <button
              class="arrow-btn"
              :disabled="idx === 0 || loading"
              aria-label="Move up"
              @click="onMoveUp(idx)"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M2 7L5 3L8 7" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </button>
            <button
              class="arrow-btn"
              :disabled="idx === state.watches.length - 1 || loading"
              aria-label="Move down"
              @click="onMoveDown(idx)"
            >
              <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                <path d="M2 3L5 7L8 3" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" />
              </svg>
            </button>
          </div>
        </div>
      </div>

      <!-- Config -->
      <div v-if="activeTab === 'actions'" class="tab-content">
        <div class="row">
          <span class="row-label">Theme</span>
          <div class="theme-picker">
            <button
              v-for="opt in themeOptions"
              :key="opt.value"
              class="theme-btn"
              :class="{ 'theme-btn--active': currentTheme === opt.value }"
              @click="setTheme(opt.value)"
            >{{ opt.label }}</button>
          </div>
        </div>
        <div class="row" title="Allow watches to run shell commands (scripts). You are responsible for any commands you configure.">
          <span class="row-label">Allow scripts</span>
          <button
            class="toggle-btn"
            :class="{ 'toggle-btn--on': allowScripts, 'toggle-btn--warn': allowScripts }"
            type="button"
            :aria-pressed="allowScripts"
            @click="onToggleAllowScripts"
          >
            <span class="toggle-thumb" />
          </button>
        </div>
        <div v-if="allowScripts" class="setting-warn">
          Scripts can execute arbitrary commands on your system. Only enable this if you trust your config.
        </div>
        <div class="row">
          <span class="row-label">Debug logging</span>
          <button
            class="toggle-btn"
            :class="{ 'toggle-btn--on': debugMode }"
            type="button"
            :aria-pressed="debugMode"
            @click="onToggleDebug"
          >
            <span class="toggle-thumb" />
          </button>
        </div>
        <div class="actions-divider" />
        <button class="action-btn" @click="onOpenConfig">
          Open config file
        </button>
        <button class="action-btn" @click="onOpenConfigDir">
          Open config folder
        </button>
        <button class="action-btn" @click="onReloadConfig" :disabled="loading">
          {{ loading ? "Reloading..." : "Reload config" }}
        </button>
      </div>

      <!-- About -->
      <div v-if="activeTab === 'about'" class="tab-content about-content">
        <div class="about-header">
          <div class="about-logo">W</div>
          <div class="about-name">wtch</div>
          <div class="about-version">v{{ appVersion }}</div>
        </div>
        <div class="about-desc">
          Lightweight system tray app for monitoring cloud services.
        </div>
        <div class="actions-divider" />
        <button class="action-btn" @click="openUrl('https://github.com/ysaunier/wtch')">
          GitHub
        </button>
        <button class="action-btn" @click="openUrl('https://github.com/ysaunier/wtch/issues')">
          Report an issue
        </button>
        <div class="actions-divider" />
        <button class="action-btn" @click="openUrl('https://ysaunier.dev')">
          ysaunier.dev
        </button>
        <button class="action-btn about-btn-coffee" @click="openUrl('https://buymeacoffee.com/ysaunier')">
          Buy me a coffee
        </button>
        <div class="about-footer">
          Made by Yoann Saunier
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-panel {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  height: 440px;
  flex-shrink: 0;
}

.tabs {
  display: flex;
  border-bottom: 1px solid var(--border);
}

.tab {
  flex: 1;
  padding: 6px 0;
  background: none;
  border: none;
  border-bottom: 2px solid transparent;
  font-size: 11px;
  font-weight: 500;
  color: var(--text-secondary);
  cursor: pointer;
  text-align: center;
}

.tab:hover {
  color: var(--text);
}

.tab--active {
  color: var(--accent);
  border-bottom-color: var(--accent);
}

.tab-body {
  padding: 8px 12px 12px;
  overflow-y: auto;
  flex: 1;
  min-height: 0;
  scrollbar-width: thin;
  scrollbar-color: var(--border) transparent;
}

.tab-body::-webkit-scrollbar {
  width: 6px;
}

.tab-body::-webkit-scrollbar-track {
  background: transparent;
}

.tab-body::-webkit-scrollbar-thumb {
  background: var(--border);
  border-radius: 3px;
}

.tab-body::-webkit-scrollbar-thumb:hover {
  background: var(--text-secondary);
}

.tab-content {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.search-input {
  width: 100%;
  padding: 5px 8px;
  margin-bottom: 6px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text);
  font-size: 12px;
  outline: none;
}

.search-input::placeholder {
  color: var(--text-secondary);
  opacity: 0.6;
}

.search-input:focus {
  border-color: var(--accent);
}

.empty-msg {
  font-size: 12px;
  color: var(--text-secondary);
  text-align: center;
  padding: 8px 0;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 4px 0;
}

.row-label {
  font-size: 12px;
  color: var(--text);
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-right: 8px;
}

.toggle-btn {
  position: relative;
  width: 28px;
  height: 16px;
  background: var(--border);
  border: none;
  border-radius: 8px;
  cursor: pointer;
  transition: background 0.15s ease;
  flex-shrink: 0;
}

.toggle-btn--on {
  background: var(--accent);
}

.toggle-btn--loading {
  opacity: 0.5;
  cursor: wait;
}

.toggle-thumb {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 12px;
  height: 12px;
  background: var(--text);
  border-radius: 50%;
  transition: transform 0.15s ease;
}

.toggle-btn--on .toggle-thumb {
  transform: translateX(12px);
}

.arrow-group {
  display: flex;
  gap: 2px;
  flex-shrink: 0;
}

.arrow-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 18px;
  background: none;
  border: none;
  border-radius: 3px;
  color: var(--text-secondary);
  cursor: pointer;
}

.arrow-btn:hover:not(:disabled) {
  background: var(--border);
  color: var(--text);
}

.arrow-btn:disabled {
  opacity: 0.25;
  cursor: default;
}

.action-btn {
  width: 100%;
  padding: 6px 10px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 4px;
  color: var(--text);
  font-size: 12px;
  cursor: pointer;
  text-align: left;
  margin-bottom: 4px;
}

.action-btn:hover:not(:disabled) {
  background: var(--border);
}

.action-btn:disabled {
  opacity: 0.5;
  cursor: wait;
}

.theme-picker {
  display: flex;
  gap: 1px;
  background: var(--border);
  border-radius: 4px;
  overflow: hidden;
}

.theme-btn {
  padding: 3px 8px;
  background: var(--surface);
  border: none;
  font-size: 11px;
  color: var(--text-secondary);
  cursor: pointer;
}

.theme-btn:hover {
  color: var(--text);
}

.theme-btn--active {
  background: var(--accent);
  color: white;
}

.toggle-btn--warn {
  background: var(--warning);
}

.setting-warn {
  font-size: 10px;
  color: var(--warning);
  padding: 2px 0 4px;
  line-height: 1.4;
}

.actions-divider {
  height: 1px;
  background: var(--border);
  margin: 6px 0;
}

.about-content {
  align-items: center;
}

.about-header {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 4px;
  padding: 12px 0 8px;
}

.about-logo {
  width: 48px;
  height: 48px;
  background: var(--bg);
  border: 2px solid var(--border);
  border-radius: 10px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 24px;
  font-weight: 700;
  color: var(--text);
}

.about-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--text);
}

.about-version {
  font-size: 11px;
  color: var(--text-secondary);
}

.about-desc {
  font-size: 11px;
  color: var(--text-secondary);
  text-align: center;
  padding: 0 8px 4px;
}

.about-btn-coffee {
  background: #ffdd00;
  border-color: #ffdd00;
  color: #1a1a1a;
  font-weight: 500;
  text-align: center;
}

.about-btn-coffee:hover {
  background: #e6c800;
  border-color: #e6c800;
}

.about-footer {
  font-size: 10px;
  color: var(--text-secondary);
  opacity: 0.5;
  text-align: center;
  padding-top: 8px;
}
</style>
