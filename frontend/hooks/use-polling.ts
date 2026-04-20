"use client";

import { useCallback, useEffect, useRef, useState } from "react";

export function usePolling<T>(
  fetcher: (signal?: AbortSignal) => Promise<T>,
  interval: number,
  enabled = true
) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const pendingRef = useRef<boolean>(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const generationRef = useRef<number>(0);
  const abortControllerRef = useRef<AbortController | null>(null);
  const enabledRef = useRef<boolean>(enabled);

  enabledRef.current = enabled;

  const execute = useCallback(async () => {
    if (pendingRef.current) return;
    pendingRef.current = true;
    setIsLoading(true);

    abortControllerRef.current?.abort();
    const controller = new AbortController();
    abortControllerRef.current = controller;

    const gen = ++generationRef.current;
    try {
      const result = await fetcher(controller.signal);
      if (gen === generationRef.current) {
        setData(result);
        setError(null);
      }
    } catch (err) {
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
      if (gen === generationRef.current) {
        setError(err instanceof Error ? err : new Error(String(err)));
      }
    } finally {
      pendingRef.current = false;
      if (gen === generationRef.current) {
        setIsLoading(false);
      }
    }
  }, [fetcher]);

  useEffect(() => {
    if (!enabled) {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      abortControllerRef.current?.abort();
      return;
    }

    const tick = async () => {
      await execute();
      if (!enabledRef.current) return;
      timeoutRef.current = setTimeout(tick, interval);
    };

    tick();

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = null;
      }
      abortControllerRef.current?.abort();
    };
  }, [enabled, execute, interval]);

  return { data, error, isLoading, refetch: execute };
}
