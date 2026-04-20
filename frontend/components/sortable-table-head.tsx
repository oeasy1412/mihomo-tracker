"use client";

import { ArrowUpDown, ArrowUp, ArrowDown } from "lucide-react";
import { Button } from "@/components/ui/button";

interface SortIconProps {
  column: string;
  sortBy: string;
  sortOrder: "asc" | "desc";
}

export function SortIcon({ column, sortBy, sortOrder }: SortIconProps) {
  if (sortBy !== column) return <ArrowUpDown className="ml-1 h-3 w-3" />;
  const Icon = sortOrder === "desc" ? ArrowDown : ArrowUp;
  return <Icon className="ml-1 h-3 w-3" />;
}

interface SortableTableHeadProps {
  label: string;
  column: string;
  sortBy: string;
  sortOrder: "asc" | "desc";
  isLoading?: boolean;
  onSort: (column: string) => void;
  align?: "left" | "right";
  className?: string;
}

export function SortableTableHead({
  label,
  column,
  sortBy,
  sortOrder,
  isLoading,
  onSort,
  align = "left",
  className,
}: SortableTableHeadProps) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className={className || "h-auto px-0 font-medium"}
      disabled={isLoading}
      onClick={() => onSort(column)}
    >
      <span className={align === "right" ? "flex items-center justify-end" : "flex items-center"}>
        {label} <SortIcon column={column} sortBy={sortBy} sortOrder={sortOrder} />
      </span>
    </Button>
  );
}
