/// <reference types="vite/client" />

interface Window {
  /** Tauri internals object - present when running inside Tauri WebView. */
  __TAURI_INTERNALS__?: unknown;
}

declare module "*.vue" {
  import type { DefineComponent } from "vue";
  const component: DefineComponent<object, object, unknown>;
  export default component;
}
