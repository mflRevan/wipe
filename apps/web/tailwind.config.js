/** @type {import('tailwindcss').Config} */

/** Builds a Tailwind color that resolves an `--wp-*-rgb` CSS variable and
 * supports the standard `/opacity` modifier syntax (e.g. `bg-card/50`). */
function withOpacity(variable) {
  return ({ opacityValue }) => {
    if (opacityValue === undefined) {
      return `rgb(var(${variable}))`;
    }
    return `rgb(var(${variable}) / ${opacityValue})`;
  };
}

export default {
  darkMode: ["selector", '[data-theme="dark"]'],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    container: {
      center: true,
      padding: "1.5rem",
      screens: { "2xl": "1200px" },
    },
    extend: {
      colors: {
        border: withOpacity("--wp-border-rgb"),
        "border-strong": withOpacity("--wp-border-strong-rgb"),
        input: withOpacity("--wp-border-rgb"),
        ring: withOpacity("--wp-focus-rgb"),
        background: withOpacity("--wp-canvas-rgb"),
        foreground: withOpacity("--wp-text-rgb"),
        primary: {
          DEFAULT: withOpacity("--wp-accent-rgb"),
          hover: withOpacity("--wp-accent-hover-rgb"),
          foreground: withOpacity("--wp-on-accent-rgb"),
        },
        secondary: {
          DEFAULT: withOpacity("--wp-surface-rgb"),
          foreground: withOpacity("--wp-text-rgb"),
        },
        muted: {
          DEFAULT: withOpacity("--wp-surface-rgb"),
          foreground: withOpacity("--wp-text-muted-rgb"),
        },
        subtle: withOpacity("--wp-text-subtle-rgb"),
        accent: {
          DEFAULT: withOpacity("--wp-accent-rgb"),
          hover: withOpacity("--wp-accent-hover-rgb"),
          foreground: withOpacity("--wp-on-accent-rgb"),
        },
        card: {
          DEFAULT: withOpacity("--wp-card-rgb"),
          foreground: withOpacity("--wp-text-rgb"),
        },
        elevated: withOpacity("--wp-elevated-rgb"),
        destructive: {
          DEFAULT: withOpacity("--wp-error-rgb"),
          foreground: withOpacity("--wp-on-accent-rgb"),
        },
        focus: withOpacity("--wp-focus-rgb"),
        error: withOpacity("--wp-error-rgb"),
      },
      borderRadius: {
        sm: "6px",
        md: "8px",
        lg: "12px",
        pill: "999px",
      },
      boxShadow: {
        DEFAULT: "var(--wp-shadow)",
        card: "var(--wp-shadow)",
        lift: "var(--wp-shadow-lift)",
      },
      fontFamily: {
        display: [
          "Space Grotesk Variable",
          "ui-sans-serif",
          "system-ui",
          "sans-serif",
        ],
        sans: [
          "Geist Variable",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "Segoe UI",
          "Roboto",
          "sans-serif",
        ],
        mono: [
          "Geist Mono Variable",
          "ui-monospace",
          "SFMono-Regular",
          "Menlo",
          "Consolas",
          "Liberation Mono",
          "monospace",
        ],
      },
      transitionTimingFunction: {
        "wp-ease": "cubic-bezier(0.2, 0, 0, 1)",
      },
      transitionDuration: {
        "wp-fast": "120ms",
        "wp-base": "160ms",
        "wp-slow": "220ms",
      },
      keyframes: {
        "fade-up": {
          "0%": { opacity: "0", transform: "translateY(8px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
      },
      animation: {
        "fade-up": "fade-up 0.5s cubic-bezier(0.2, 0, 0, 1) both",
      },
    },
  },
  plugins: [require("tailwindcss-animate")],
};
