"use client";

import { Pagination } from "./pagination";
import { PageSizeSelect } from "./page-size-select";

interface TablePaginationFooterProps {
  total: number;
  currentPageSize: number;
  page: number;
  hasPrev: boolean;
  hasNext: boolean;
  isLoading?: boolean;
  onPageChange: (page: number) => void;
  onPageSizeChange: (size: number) => void;
}

export function TablePaginationFooter({
  total,
  currentPageSize,
  page,
  hasPrev,
  hasNext,
  isLoading,
  onPageChange,
  onPageSizeChange,
}: TablePaginationFooterProps) {
  return (
    <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between text-sm">
      <div className="flex items-center gap-4">
        <span className="text-muted-foreground">
          共 {total.toLocaleString("zh-CN")} 条记录，本页 {currentPageSize} 条
        </span>
        <div className="flex items-center gap-2">
          <span className="text-muted-foreground">每页</span>
          <PageSizeSelect
            value={currentPageSize}
            onChange={onPageSizeChange}
            disabled={isLoading}
          />
        </div>
      </div>
      <Pagination
        page={page}
        hasPrev={hasPrev}
        hasNext={hasNext}
        isLoading={isLoading}
        onPageChange={onPageChange}
      />
    </div>
  );
}
