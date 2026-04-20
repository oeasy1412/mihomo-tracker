"use client";

import { createContext, useContext, useEffect, useState, ReactNode } from "react";
import { fetchTimezone, type TimezoneInfo } from "@/lib/api";
import { useApiConfig } from "@/hooks/use-api-config";

interface TimezoneContextValue {
  timezone: TimezoneInfo | null;
  isLoading: boolean;
  error: string | null;
}

const TimezoneContext = createContext<TimezoneContextValue | undefined>(undefined);

export function TimezoneProvider({ children }: { children: ReactNode }) {
  const { isConfigured } = useApiConfig();
  const [timezone, setTimezone] = useState<TimezoneInfo | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isConfigured) return;

    let cancelled = false;
    setIsLoading(true);
    setError(null);

    fetchTimezone()
      .then((tz) => {
        if (!cancelled) {
          setTimezone(tz);
          setError(null);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
        }
      })
      .finally(() => {
        if (!cancelled) {
          setIsLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [isConfigured]);

  return (
    <TimezoneContext.Provider value={{ timezone, isLoading, error }}>
      {children}
    </TimezoneContext.Provider>
  );
}

export function useMasterTimezone() {
  const context = useContext(TimezoneContext);
  if (!context) {
    throw new Error("useMasterTimezone must be used within TimezoneProvider");
  }
  return context;
}