"use client";

import { useMemo } from "react";
import { usePolling } from "@/hooks/use-polling";
import { useApiConfig } from "@/hooks/use-api-config";

export function useApiPolling<T>(
  fetcher: (signal?: AbortSignal) => Promise<T>,
  interval: number,
  enabled = true
) {
  const { isConfigured } = useApiConfig();
  const pollingEnabled = useMemo(() => enabled && isConfigured, [enabled, isConfigured]);

  return {
    ...usePolling(fetcher, interval, pollingEnabled),
    isConfigured,
  };
}
