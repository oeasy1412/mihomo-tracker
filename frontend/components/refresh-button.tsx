"use client";

import { RefreshCw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface RefreshButtonProps {
  onClick: () => void;
  isLoading: boolean;
  size?: "default" | "icon" | "sm";
  label?: string;
}

export function RefreshButton({
  onClick,
  isLoading,
  size = "icon",
  label = "刷新",
}: RefreshButtonProps) {
  if (size === "sm") {
    return (
      <Button variant="outline" size="sm" onClick={onClick} disabled={isLoading}>
        <RefreshCw className={cn("mr-2 h-4 w-4", isLoading && "animate-spin")} />
        {label}
      </Button>
    );
  }

  return (
    <Button variant="outline" size="icon" onClick={onClick} disabled={isLoading}>
      <RefreshCw className={cn("h-4 w-4", isLoading && "animate-spin")} />
    </Button>
  );
}
