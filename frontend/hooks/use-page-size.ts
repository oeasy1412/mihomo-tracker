"use client";

import { useState, useCallback } from "react";

const VALID_PAGE_SIZES = [20, 50, 100] as const;
export type PageSize = (typeof VALID_PAGE_SIZES)[number];

export function usePageSize(defaultSize: PageSize = 20) {
  const [limit, setLimit] = useState<PageSize>(defaultSize);
  const [page, setPage] = useState(0);

  const handleLimitChange = useCallback((value: number) => {
    if ((VALID_PAGE_SIZES as readonly number[]).includes(value)) {
      setLimit(value as PageSize);
      setPage(0);
    }
  }, []);

  return { limit, page, setPage, handleLimitChange };
}