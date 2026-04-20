"use client";

import { createContext, useContext, ReactNode } from "react";

interface SettingsContextValue {
  openSettings: () => void;
}

const SettingsContext = createContext<SettingsContextValue | undefined>(
  undefined
);

export function SettingsProvider({
  children,
  openSettings,
}: {
  children: ReactNode;
  openSettings: () => void;
}) {
  return (
    <SettingsContext.Provider value={{ openSettings }}>
      {children}
    </SettingsContext.Provider>
  );
}

export function useSettingsDialog() {
  const context = useContext(SettingsContext);
  if (!context) {
    throw new Error("useSettingsDialog must be used within SettingsProvider");
  }
  return context;
}
