import { useEffect, useState } from "react";
import { Moon, Sun } from "lucide-react";
import { applyTheme, getStoredTheme, type Theme } from "@/lib/theme";

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(getStoredTheme);

  useEffect(() => {
    applyTheme(theme);
  }, [theme]);

  function toggle() {
    // Cross-fade the whole page for the duration of the switch only.
    const root = document.documentElement;
    root.classList.add("theme-transition");
    window.setTimeout(() => root.classList.remove("theme-transition"), 260);
    setTheme((t) => (t === "dark" ? "light" : "dark"));
  }

  return (
    <button
      type="button"
      aria-label={theme === "dark" ? "Switch to light theme" : "Switch to dark theme"}
      onClick={toggle}
      className="inline-flex h-9 w-9 items-center justify-center rounded-sm text-muted-foreground transition-colors duration-wp-fast hover:bg-elevated hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-focus"
    >
      {theme === "dark" ? (
        <Sun className="h-4 w-4" />
      ) : (
        <Moon className="h-4 w-4" />
      )}
    </button>
  );
}
