import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { usePageSize } from "@/hooks/use-page-size";

describe("usePageSize", () => {
  it("initializes with default size and page 0", () => {
    const { result } = renderHook(() => usePageSize(20));
    expect(result.current.limit).toBe(20);
    expect(result.current.page).toBe(0);
  });

  it("accepts valid page sizes", () => {
    const { result } = renderHook(() => usePageSize(20));
    act(() => { result.current.handleLimitChange(50); });
    expect(result.current.limit).toBe(50);
    expect(result.current.page).toBe(0);

    act(() => { result.current.handleLimitChange(100); });
    expect(result.current.limit).toBe(100);
    expect(result.current.page).toBe(0);
  });

  it("ignores invalid page sizes", () => {
    const { result } = renderHook(() => usePageSize(20));
    act(() => { result.current.handleLimitChange(25); });
    expect(result.current.limit).toBe(20); // unchanged
    expect(result.current.page).toBe(0);
  });

  it("resets page to 0 on valid size change", () => {
    const { result } = renderHook(() => usePageSize(20));
    act(() => { result.current.setPage(5); });
    act(() => { result.current.handleLimitChange(50); });
    expect(result.current.limit).toBe(50);
    expect(result.current.page).toBe(0);
  });

  it("resets page via setPage", () => {
    const { result } = renderHook(() => usePageSize(20));
    act(() => { result.current.setPage(3); });
    expect(result.current.page).toBe(3);
  });
});