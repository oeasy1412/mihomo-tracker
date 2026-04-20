"use client";

import { useCallback, useEffect, useState } from "react";
import {
  API_CONFIG_STORAGE_KEY,
  API_CONFIG_UPDATED_EVENT,
  getApiConfig,
  setApiConfig,
} from "@/lib/api";

export interface ApiConfigState {
  baseUrl: string;
  token: string;
  isConfigured: boolean;
  isFirstVisit: boolean;
}

function readApiConfigState(): ApiConfigState {
  const raw = getApiConfig();
  const hasConfig = Boolean(raw.baseUrl);
  return {
    baseUrl: raw.baseUrl || "",
    token: raw.token || "",
    isConfigured: hasConfig,
    isFirstVisit: !hasConfig,
  };
}

export function useApiConfig() {
  const [config, setConfigState] = useState<ApiConfigState>({
    baseUrl: "",
    token: "",
    isConfigured: false,
    isFirstVisit: false,
  });

  useEffect(() => {
    const syncConfig = () => {
      setConfigState(readApiConfigState());
    };
    const handleStorage = (event: StorageEvent) => {
      if (event.key === API_CONFIG_STORAGE_KEY) {
        syncConfig();
      }
    };

    syncConfig();
    window.addEventListener(API_CONFIG_UPDATED_EVENT, syncConfig);
    window.addEventListener("storage", handleStorage);

    return () => {
      window.removeEventListener(API_CONFIG_UPDATED_EVENT, syncConfig);
      window.removeEventListener("storage", handleStorage);
    };
  }, []);

  const saveConfig = useCallback((baseUrl: string, token: string) => {
    const normalized = baseUrl.trim().replace(/\/$/, "");
    setApiConfig({ baseUrl: normalized, token });
    setConfigState({
      baseUrl: normalized,
      token,
      isConfigured: Boolean(normalized),
      isFirstVisit: false,
    });
  }, []);

  const clearConfig = useCallback(() => {
    setApiConfig({ baseUrl: "", token: "" });
    setConfigState({
      baseUrl: "",
      token: "",
      isConfigured: false,
      isFirstVisit: true,
    });
  }, []);

  return {
    ...config,
    saveConfig,
    clearConfig,
  };
}
