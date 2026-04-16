<script setup lang="ts">
import { useWatchState } from "./composables/useWatchState";
import { useTheme } from "./composables/useTheme";
import WatchList from "./components/WatchList.vue";

const { state, refresh, timeSinceLastCheck } = useWatchState();
useTheme();
</script>

<template>
  <main>
    <WatchList
      :state="state"
      :time-since-last-check="timeSinceLastCheck"
      @refresh="refresh"
    />
  </main>
</template>

<style>
:root,
[data-theme="dark"] {
  --bg: #1a1a2e;
  --surface: #16213e;
  --text: #e0e0e0;
  --text-secondary: #8a8a8a;
  --accent: #4caf50;
  --success: #4caf50;
  --warning: #ff9800;
  --error: #f44336;
  --maintenance: #64b5f6;
  --unknown: #9e9e9e;
  --border: #2a2a4a;
}

[data-theme="light"] {
  --bg: #f5f5f5;
  --surface: #ffffff;
  --text: #1a1a1a;
  --text-secondary: #666666;
  --accent: #388e3c;
  --success: #388e3c;
  --warning: #f57c00;
  --error: #d32f2f;
  --maintenance: #1e88e5;
  --unknown: #9e9e9e;
  --border: #e0e0e0;
}

* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

html, body {
  height: 100%;
  overflow: hidden;
}

body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
  font-size: 14px;
  color: var(--text);
  background: transparent;
  width: 320px;
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
}

#app {
  display: flex;
  flex-direction: column;
  justify-content: flex-end;
  flex: 1;
  min-height: 0;
}

main {
  display: flex;
  flex-direction: column;
  background: var(--bg);
  border-radius: 8px;
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.3);
  overflow: hidden;
  max-height: calc(100vh - 8px);
}

main + .arrow {
  display: none;
}

body::after {
  content: "";
  display: block;
  width: 0;
  height: 0;
  margin: 0 auto;
  flex-shrink: 0;
  border-left: 8px solid transparent;
  border-right: 8px solid transparent;
  border-top: 8px solid var(--bg);
}
</style>
