import { describe, it, expect, vi } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { usePolling } from "@/hooks/use-polling";

describe("usePolling", () => {
  it("fetches data on mount", async () => {
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    const { result } = renderHook(() => usePolling(fetcher, 5000));

    expect(result.current.isLoading).toBe(true);

    await waitFor(() => expect(result.current.data).toEqual({ count: 1 }));
    expect(result.current.error).toBeNull();
    expect(fetcher).toHaveBeenCalledTimes(1);
  });

  it("sets error when fetcher rejects", async () => {
    const fetcher = vi.fn().mockRejectedValue(new Error("fetch failed"));
    const { result } = renderHook(() => usePolling(fetcher, 5000));

    await waitFor(() => expect(result.current.error).toBeInstanceOf(Error));
    expect(result.current.error?.message).toBe("fetch failed");
    expect(result.current.data).toBeNull();
  });

  it("polls on interval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    renderHook(() => usePolling(fetcher, 1000));

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));

    act(() => {
      vi.advanceTimersByTime(1000);
    });
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));

    vi.useRealTimers();
  });

  it("does not overlap concurrent fetches", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    let resolve: (() => void) | undefined;
    const fetcher = vi.fn().mockImplementation(
      () =>
        new Promise<void>((res) => {
          resolve = res;
        })
    );

    const { result } = renderHook(() => usePolling(fetcher, 500));
    expect(result.current.isLoading).toBe(true);

    // trigger another tick while still pending
    act(() => {
      vi.advanceTimersByTime(500);
    });
    expect(fetcher).toHaveBeenCalledTimes(1);

    act(() => {
      resolve?.();
    });
    await waitFor(() => expect(result.current.isLoading).toBe(false));

    // next tick should trigger again
    act(() => {
      vi.advanceTimersByTime(500);
    });
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));

    vi.useRealTimers();
  });

  it("does not fetch when enabled is false", async () => {
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    renderHook(() => usePolling(fetcher, 5000, false));

    // wait a bit to ensure no fetch happens
    await new Promise((r) => setTimeout(r, 50));
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("refetch triggers manual fetch", async () => {
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    const { result } = renderHook(() => usePolling(fetcher, 5000));

    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));

    act(() => {
      result.current.refetch();
    });
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(2));
  });

  it("aborts in-flight request on unmount", async () => {
    let capturedSignal: AbortSignal | undefined;
    const fetcher = vi.fn().mockImplementation((signal?: AbortSignal) => {
      capturedSignal = signal;
      return new Promise(() => {});
    });
    const { unmount } = renderHook(() => usePolling(fetcher, 5000));
    await waitFor(() => expect(fetcher).toHaveBeenCalled());
    expect(capturedSignal).toBeDefined();
    expect(capturedSignal!.aborted).toBe(false);
    unmount();
    expect(capturedSignal!.aborted).toBe(true);
  });
});
