"use client";

import { XCircle } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ErrorBannerProps {
  message: string;
  onRetry?: () => void;
  onDismiss?: () => void;
  isLoading?: boolean;
}

export function ErrorBanner({ message, onRetry, onDismiss, isLoading }: ErrorBannerProps) {
  return (
    <div className="flex items-center justify-between gap-4 rounded-md border border-destructive/50 bg-destructive/10 px-4 py-3 text-sm text-destructive">
      <div className="flex items-center gap-2">
        <XCircle className="h-4 w-4 shrink-0" />
        <span>{message}</span>
      </div>
      <div className="flex items-center gap-2">
        {onRetry && (
          <Button variant="ghost" size="sm" onClick={onRetry} disabled={isLoading}>
            重试
          </Button>
        )}
        {onDismiss && (
          <Button variant="ghost" size="sm" onClick={onDismiss}>
            忽略
          </Button>
        )}
      </div>
    </div>
  );
}
