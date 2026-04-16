/** Status of a single watch. */
export type WatchStatus = "success" | "warning" | "error" | "maintenance" | "unknown";

/** Visual display style for a watch or detail row. */
export type DisplayStyle = "status" | "progress" | "value" | "text" | "list";

/** A single item in a list-style display. */
export interface ListItem {
  title: string;
  status: WatchStatus;
}

/** How a watch should be rendered in the popup. */
export interface WatchDisplay {
  style: DisplayStyle;
  title?: string;
  description?: string;
  content?: string;
  value?: string;
  min?: number;
  max?: number;
  items?: ListItem[];
}

/** A detail row shown when a watch is expanded. */
export interface WatchDetail {
  title: string;
  style: DisplayStyle;
  description?: string;
  value?: string | number;
  min?: number;
  max?: number;
}

/** Full state of a single watch. */
export interface WatchState {
  name: string;
  status: WatchStatus;
  display: WatchDisplay;
  details: WatchDetail[];
  expand: boolean;
  url?: string;
  lastCheck?: string;
}

/** Top-level application state returned by the Tauri backend. */
export interface AppState {
  watches: WatchState[];
  lastCheck?: string;
  groups?: Record<string, string[]>;
}

/** A preset that can be enabled or disabled from the settings panel. */
export interface PresetInfo {
  name: string;
  enabled: boolean;
}
