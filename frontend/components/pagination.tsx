"use client";

import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button } from "@/components/ui/button";

interface PaginationProps {
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  isLoading?: boolean;
  onPageChange: (page: number) => void;
  labels?: { prev?: string; next?: string };
}

export function Pagination({
  page,
  hasPrev,
  hasNext,
  isLoading,
  onPageChange,
  labels = { prev: "上一页", next: "下一页" },
}: PaginationProps) {
  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        disabled={!hasPrev || isLoading}
        onClick={() => onPageChange(page - 1)}
      >
        {labels?.prev || "上一页"}
      </Button>
      <span className="text-sm">第 {page + 1} 页</span>
      <Button
        variant="outline"
        size="sm"
        disabled={!hasNext || isLoading}
        onClick={() => onPageChange(page + 1)}
      >
        {labels?.next || "下一页"}
      </Button>
    </div>
  );
}

interface IconPaginationProps {
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  isLoading?: boolean;
  onPageChange: (page: number) => void;
}

export function IconPagination({
  page,
  hasPrev,
  hasNext,
  isLoading,
  onPageChange,
}: IconPaginationProps) {
  return (
    <div className="flex items-center gap-2">
      <Button
        variant="outline"
        size="sm"
        disabled={!hasPrev || isLoading}
        onClick={() => onPageChange(page - 1)}
      >
        <ChevronLeft className="h-4 w-4" />
      </Button>
      <span className="text-sm">第 {page + 1} 页</span>
      <Button
        variant="outline"
        size="sm"
        disabled={!hasNext || isLoading}
        onClick={() => onPageChange(page + 1)}
      >
        <ChevronRight className="h-4 w-4" />
      </Button>
    </div>
  );
}
