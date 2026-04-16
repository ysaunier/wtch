import { ref, watch, onMounted } from "vue";

export type Theme = "system" | "dark" | "light";

const STORAGE_KEY = "wtch-theme";

const currentTheme = ref<Theme>("system");

function getSystemPreference(): "dark" | "light" {
  return window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

function applyTheme(theme: Theme): void {
  const resolved = theme === "system" ? getSystemPreference() : theme;
  document.documentElement.setAttribute("data-theme", resolved);
}

export function useTheme() {
  onMounted(() => {
    const stored = localStorage.getItem(STORAGE_KEY) as Theme | null;
    if (stored && ["system", "dark", "light"].includes(stored)) {
      currentTheme.value = stored;
    }
    applyTheme(currentTheme.value);

    window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
      if (currentTheme.value === "system") {
        applyTheme("system");
      }
    });
  });

  watch(currentTheme, (theme) => {
    localStorage.setItem(STORAGE_KEY, theme);
    applyTheme(theme);
  });

  function setTheme(theme: Theme): void {
    currentTheme.value = theme;
  }

  return { currentTheme, setTheme };
}
