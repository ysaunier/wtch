import { computed, reactive, ref, onMounted } from "vue";
import type { AppState } from "../types";

function secondsSince(iso: string): number {
  return Math.floor((Date.now() - new Date(iso).getTime()) / 1000);
}

function formatAge(seconds: number): string {
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  return `${Math.floor(seconds / 3600)}h ago`;
}

async function tauriInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (window.__TAURI_INTERNALS__) {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<T>(cmd, args);
  }
  return null;
}

export function useWatchState() {
  const state = reactive<AppState>({
    watches: [],
    lastCheck: undefined,
  });
  const tickCount = ref(0);

  setInterval(() => {
    tickCount.value++;
  }, 1000);

  const timeSinceLastCheck = computed<string>(() => {
    void tickCount.value;
    if (!state.lastCheck) return "...";
    return formatAge(secondsSince(state.lastCheck));
  });

  async function fetchState(): Promise<void> {
    const result = await tauriInvoke<AppState>("get_state");
    if (result) {
      state.watches = result.watches;
      state.lastCheck = result.lastCheck;
      state.groups = result.groups;
    }
  }

  async function refresh(): Promise<void> {
    await tauriInvoke("refresh");
    await fetchState();
  }

  onMounted(async () => {
    await fetchState();
    // Poll for state updates every 5 seconds
    setInterval(fetchState, 5000);
  });

  return { state, refresh, timeSinceLastCheck };
}
