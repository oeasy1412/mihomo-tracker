"use client";

import { useState, useCallback } from "react";

export type SortOrder = "asc" | "desc";

export function useSort<T extends string>(
  defaultColumn: T,
  defaultOrder: SortOrder = "desc"
) {
  const [sortBy, setSortBy] = useState<T>(defaultColumn);
  const [sortOrder, setSortOrder] = useState<SortOrder>(defaultOrder);
  const [page, setPage] = useState(0);

  const handleSort = useCallback(
    (column: string) => {
      if (sortBy === column) {
        setSortOrder((prev) => (prev === "desc" ? "asc" : "desc"));
      } else {
        setSortBy(column as T);
        setSortOrder("desc");
      }
      setPage(0);
    },
    [sortBy]
  );

  const resetPage = useCallback(() => setPage(0), []);

  return { sortBy, sortOrder, page, setPage, handleSort, resetPage };
}