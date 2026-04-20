"use client";

import { useEffect, useState } from "react";
import { useApiConfig } from "@/hooks/use-api-config";
import { SettingsDialog } from "@/components/settings-dialog";
import { ErrorBanner } from "@/components/error-banner";
import { Button } from "@/components/ui/button";
import { SettingsProvider } from "./settings-context";
import { TimezoneProvider } from "./timezone-context";

export function ClientLayout({ children }: { children: React.ReactNode }) {
  const { baseUrl, token, isConfigured, isFirstVisit, saveConfig } =
    useApiConfig();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [apiError, setApiError] = useState<string | null>(null);

  useEffect(() => {
    if (isFirstVisit) {
      setSettingsOpen(true);
    }
  }, [isFirstVisit]);

  // Simple global error listener for demonstration
  useEffect(() => {
    const handler = (e: ErrorEvent) => {
      // In a real app, you'd wire this to your API error reporting
      console.error(e.error);
    };
    window.addEventListener("error", handler);
    return () => window.removeEventListener("error", handler);
  }, []);

  return (
    <SettingsProvider openSettings={() => setSettingsOpen(true)}>
      <TimezoneProvider>
        {apiError && (
          <div className="fixed left-0 right-0 top-0 z-50 px-4 py-2">
            <ErrorBanner
              message={apiError}
              onDismiss={() => setApiError(null)}
            />
          </div>
        )}
        <div className="flex min-h-screen flex-col">
          {!isConfigured && !settingsOpen && (
            <div className="border-b bg-amber-50 px-4 py-2 text-sm text-amber-800 dark:bg-amber-950 dark:text-amber-200">
              <div className="mx-auto flex max-w-7xl items-center justify-between">
                <span>尚未配置 API 地址，请点击设置进行配置</span>
                <Button variant="link" size="sm" onClick={() => setSettingsOpen(true)}>
                  设置
                </Button>
              </div>
            </div>
          )}
          <div className="flex-1">{children}</div>
        </div>
        <SettingsDialog
          open={settingsOpen}
          onOpenChange={setSettingsOpen}
          defaultBaseUrl={baseUrl}
          defaultToken={token}
          onSave={saveConfig}
        />
      </TimezoneProvider>
    </SettingsProvider>
  );
}
