"use client";

import {
  createContext,
  useContext,
  useEffect,
  useState,
  ReactNode,
} from "react";

type Theme = "dark" | "light" | "system";

interface ThemeProviderProps {
  children: ReactNode;
  defaultTheme?: Theme;
  enableSystem?: boolean;
  disableTransitionOnChange?: boolean;
}

interface ThemeProviderState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  resolvedTheme: "dark" | "light";
}

const ThemeProviderContext = createContext<ThemeProviderState | undefined>(
  undefined
);

export function ThemeProvider({
  children,
  defaultTheme = "system",
  enableSystem = true,
  disableTransitionOnChange = false,
}: ThemeProviderProps) {
  const [theme, setThemeState] = useState<Theme>(defaultTheme);
  const [resolvedTheme, setResolvedTheme] = useState<"dark" | "light">("light");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
    const saved = (() => {
      try {
        return localStorage.getItem("mihomo-theme") as Theme | null;
      } catch (e) {
        console.warn("无法读取主题设置:", e);
        return null;
      }
    })();
    if (saved) {
      setThemeState(saved);
    }
  }, []);

  useEffect(() => {
    if (!mounted) return;

    const root = window.document.documentElement;
    root.classList.remove("light", "dark");

    if (disableTransitionOnChange) {
      const style = document.createElement("style");
      style.innerHTML =
        "*,*::before,*::after{transition:none!important;animation:none!important}";
      document.head.appendChild(style);
      requestAnimationFrame(() => {
        document.head.removeChild(style);
      });
    }

    let resolved: "light" | "dark";
    if (theme === "system" && enableSystem) {
      resolved = window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light";
    } else {
      resolved = theme === "dark" ? "dark" : "light";
    }

    root.classList.add(resolved);
    setResolvedTheme(resolved);
  }, [theme, enableSystem, disableTransitionOnChange, mounted]);

  useEffect(() => {
    if (!mounted || theme !== "system" || !enableSystem) return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = (e: MediaQueryListEvent) => {
      const next = e.matches ? "dark" : "light";
      document.documentElement.classList.remove("light", "dark");
      document.documentElement.classList.add(next);
      setResolvedTheme(next);
    };
    mq.addEventListener("change", onChange);
    return () => mq.removeEventListener("change", onChange);
  }, [theme, enableSystem, mounted]);

  const setTheme = (newTheme: Theme) => {
    try {
      localStorage.setItem("mihomo-theme", newTheme);
    } catch (e) {
      console.warn("无法保存主题设置:", e);
    }
    setThemeState(newTheme);
  };

  return (
    <ThemeProviderContext.Provider value={{ theme, setTheme, resolvedTheme }}>
      {children}
    </ThemeProviderContext.Provider>
  );
}

export function useTheme() {
  const context = useContext(ThemeProviderContext);
  if (!context) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
}
