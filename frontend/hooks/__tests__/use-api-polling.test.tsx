import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, waitFor, act } from "@testing-library/react";
import { useApiPolling } from "@/hooks/use-api-polling";
import { setApiConfig } from "@/lib/api";

describe("useApiPolling", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("does not request when API config is missing", async () => {
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    const { result } = renderHook(() => useApiPolling(fetcher, 1000, true));

    await waitFor(() => expect(result.current.isConfigured).toBe(false));
    expect(fetcher).not.toHaveBeenCalled();
  });

  it("starts polling after API config is saved", async () => {
    const fetcher = vi.fn().mockResolvedValue({ count: 1 });
    const { result } = renderHook(() => useApiPolling(fetcher, 1000, true));

    await waitFor(() => expect(result.current.isConfigured).toBe(false));
    expect(fetcher).not.toHaveBeenCalled();

    act(() => {
      setApiConfig({ baseUrl: "http://localhost:8051", token: "" });
    });

    await waitFor(() => expect(result.current.isConfigured).toBe(true));
    await waitFor(() => expect(fetcher).toHaveBeenCalledTimes(1));
  });
});
