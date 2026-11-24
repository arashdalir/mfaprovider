import { useCallback, useEffect, useState } from "react";

import { loadTheme, resolveIsDark, saveTheme, type Theme } from "./theme";

const DARK_QUERY = "(prefers-color-scheme: dark)";

function systemPrefersDark(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia(DARK_QUERY).matches;
}

/** Applies the theme to the page and follows the system while the choice is "system". */
export function useTheme(): { theme: Theme; setTheme: (theme: Theme) => void } {
  const [theme, setThemeState] = useState<Theme>(loadTheme);

  useEffect(() => {
    const apply = () =>
      document.documentElement.classList.toggle("dark", resolveIsDark(theme, systemPrefersDark()));
    apply();
    if (typeof window.matchMedia !== "function") {
      return;
    }
    const query = window.matchMedia(DARK_QUERY);
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, [theme]);

  const setTheme = useCallback((next: Theme) => {
    saveTheme(next);
    setThemeState(next);
  }, []);

  return { theme, setTheme };
}
