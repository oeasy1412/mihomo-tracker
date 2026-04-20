import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useSort } from "@/hooks/use-sort";

describe("useSort", () => {
  it("initializes with default column and order", () => {
    const { result } = renderHook(() => useSort("download", "desc"));
    expect(result.current.sortBy).toBe("download");
    expect(result.current.sortOrder).toBe("desc");
    expect(result.current.page).toBe(0);
  });

  it("toggles sort order when same column is clicked", () => {
    const { result } = renderHook(() => useSort("download", "desc"));
    act(() => { result.current.handleSort("download"); });
    expect(result.current.sortBy).toBe("download");
    expect(result.current.sortOrder).toBe("asc");
    expect(result.current.page).toBe(0);
  });

  it("changes column and resets to desc when different column is clicked", () => {
    const { result } = renderHook(() => useSort("download", "desc"));
    act(() => { result.current.handleSort("upload"); });
    expect(result.current.sortBy).toBe("upload");
    expect(result.current.sortOrder).toBe("desc");
    expect(result.current.page).toBe(0);
  });

  it("resets page via resetPage", () => {
    const { result } = renderHook(() => useSort("download", "desc"));
    act(() => { result.current.handleSort("download"); }); // order → asc
    act(() => { result.current.resetPage(); });
    expect(result.current.page).toBe(0);
  });

  it("resets page when changing sort column", () => {
    const { result } = renderHook(() => useSort("download", "desc"));
    act(() => { result.current.setPage(3); });
    act(() => { result.current.handleSort("upload"); });
    expect(result.current.page).toBe(0);
  });
});