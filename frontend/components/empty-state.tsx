"use client";

import { TableRow, TableCell } from "@/components/ui/table";
import { cn } from "@/lib/utils";

interface EmptyStateProps {
  message?: string;
  className?: string;
  height?: "sm" | "md" | "lg" | "full";
}

export function EmptyState({
  message = "暂无数据",
  className,
  height = "md",
}: EmptyStateProps) {
  const heightClass = {
    sm: "h-24",
    md: "h-32",
    lg: "h-48",
    full: "h-full",
  }[height];

  return (
    <div
      className={cn(
        "flex items-center justify-center rounded-md border text-sm text-muted-foreground",
        heightClass,
        className
      )}
    >
      {message}
    </div>
  );
}

interface TableEmptyStateProps {
  colSpan: number;
  message?: string;
  height?: "sm" | "md" | "lg";
}

export function TableEmptyState({
  colSpan,
  message = "暂无数据",
  height = "md",
}: TableEmptyStateProps) {
  const heightClass = {
    sm: "h-24",
    md: "h-32",
    lg: "h-48",
  }[height];

  return (
    <TableRow>
      <TableCell
        colSpan={colSpan}
        className={cn(
          "text-center text-muted-foreground",
          heightClass
        )}
      >
        {message}
      </TableCell>
    </TableRow>
  );
}

interface TableLoadingRowProps {
  colSpan: number;
  height?: "sm" | "md" | "lg";
}

export function TableLoadingRow({
  colSpan,
  height = "md",
}: TableLoadingRowProps) {
  const heightClass = {
    sm: "h-24",
    md: "h-32",
    lg: "h-48",
  }[height];

  return (
    <TableRow>
      <TableCell colSpan={colSpan} className={cn("text-center", heightClass)}>
        加载中...
      </TableCell>
    </TableRow>
  );
}
