import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  ApiError,
  apiRequest,
  buildFilterParams,
  fetchConnectionLogs,
  getApiConfig,
  setApiConfig,
  checkHealth,
  logStreamSocket,
} from "@/lib/api";

describe("ApiError", () => {
  it("sets name and message", () => {
    const err = new ApiError("something went wrong", 500);
    expect(err.name).toBe("ApiError");
    expect(err.message).toBe("something went wrong");
    expect(err.status).toBe(500);
  });

  it("allows undefined status", () => {
    const err = new ApiError("network error");
    expect(err.status).toBeUndefined();
  });
});

describe("buildFilterParams", () => {
  it("includes only truthy values", () => {
    const result = buildFilterParams({
      from: "2024-01-01",
      to: null,
      agentId: "agent-1",
      network: "",
      rule: "DIRECT",
      process: null,
      source: null,
      destination: null,
      host: null,
      chains: null,
    });
    expect(result).toEqual({
      from: "2024-01-01",
      agent_id: "agent-1",
      rule: "DIRECT",
    });
  });

  it("returns empty object when all filters are empty", () => {
    const result = buildFilterParams({
      from: null,
      to: null,
      agentId: null,
      network: null,
      rule: null,
      process: null,
      source: null,
      destination: null,
      host: null,
      chains: null,
    });
    expect(result).toEqual({});
  });

  it("includes host and chains when provided", () => {
    const result = buildFilterParams({
      from: null,
      to: null,
      agentId: null,
      network: null,
      rule: null,
      process: null,
      source: null,
      destination: null,
      host: "example.com",
      chains: "node-a",
    });
    expect(result).toEqual({
      host: "example.com",
      chains: "node-a",
    });
  });

  it("includes destination_port and exclude_rule when provided", () => {
    const result = buildFilterParams({
      from: null,
      to: null,
      agentId: null,
      network: null,
      rule: null,
      process: null,
      source: null,
      destination: null,
      host: null,
      chains: null,
      destination_port: "443",
      exclude_rule: "DIRECT",
    });
    expect(result).toEqual({
      destination_port: "443",
      exclude_rule: "DIRECT",
    });
  });
});

describe("fetchConnectionLogs", () => {
  beforeEach(() => {
    localStorage.setItem(
      "mihomo-api-config",
      JSON.stringify({ baseUrl: "http://localhost:8051", token: "my-token" })
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it("builds query string with filters and pagination and returns data", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      statusText: "OK",
      text: async () => JSON.stringify({ status: "success", data: { total: 1, items: [{ id: "1", download: 100, upload: 50 }] }, message: null }),
      json: async () => ({ status: "success", data: { total: 1, items: [{ id: "1", download: 100, upload: 50 }] }, message: null }),
    } as unknown as Response);

    const result = await fetchConnectionLogs(
      {
        agentId: "agent-1",
        host: "google",
        network: "tcp",
      },
      {
        limit: 50,
        offset: 100,
        sortBy: "download",
        sortOrder: "asc",
      }
    );

    const url = (global.fetch as ReturnType<typeof vi.fn>).mock.calls[0][0] as string;
    expect(new URL(url).searchParams.get("agent_id")).toBe("agent-1");
    expect(new URL(url).searchParams.get("host")).toBe("google");
    expect(new URL(url).searchParams.get("network")).toBe("tcp");
    expect(new URL(url).searchParams.get("limit")).toBe("50");
    expect(new URL(url).searchParams.get("offset")).toBe("100");
    expect(new URL(url).searchParams.get("sort_by")).toBe("download");
    expect(new URL(url).searchParams.get("sort_order")).toBe("asc");
    expect(result).toEqual({ total: 1, items: [{ id: "1", download: 100, upload: 50 }] });
  });

  it("throws ApiError when baseUrl is not configured", async () => {
    localStorage.setItem("mihomo-api-config", JSON.stringify({ baseUrl: "", token: "" }));
    await expect(fetchConnectionLogs()).rejects.toThrow("API 地址未配置");
  });

  it("throws ApiError on network failure", async () => {
    localStorage.setItem(
      "mihomo-api-config",
      JSON.stringify({ baseUrl: "http://localhost:8051", token: "my-token" })
    );
    global.fetch = vi.fn().mockRejectedValue(new Error("net::ERR_FAILED"));
    await expect(fetchConnectionLogs()).rejects.toThrow("网络错误: net::ERR_FAILED");
  });

  it("throws ApiError when response is not ok", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
      text: async () => JSON.stringify({ status: "error", data: null, message: "server error" }),
      json: async () => ({ status: "error", data: null, message: "server error" }),
    } as unknown as Response);
    await expect(fetchConnectionLogs()).rejects.toThrow(ApiError);
    await expect(fetchConnectionLogs()).rejects.toThrow("server error");
  });

  it("throws ApiError when response body has status error", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify({ status: "error", data: null, message: "business error" }),
      json: async () => ({ status: "error", data: null, message: "business error" }),
    } as unknown as Response);
    await expect(fetchConnectionLogs()).rejects.toThrow(ApiError);
    await expect(fetchConnectionLogs()).rejects.toThrow("business error");
  });

  it("throws ApiError on invalid JSON response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => "not-json",
    } as unknown as Response);
    await expect(fetchConnectionLogs()).rejects.toThrow(ApiError);
    await expect(fetchConnectionLogs()).rejects.toThrow("无效响应");
  });
});

describe("getApiConfig / setApiConfig", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("returns empty config when localStorage is empty", () => {
    expect(getApiConfig()).toEqual({ baseUrl: "", token: "" });
  });

  it("parses stored config", () => {
    setApiConfig({ baseUrl: "http://localhost:8051", token: "secret" });
    expect(getApiConfig()).toEqual({ baseUrl: "http://localhost:8051", token: "secret" });
  });

  it("returns empty config for invalid JSON", () => {
    localStorage.setItem("mihomo-api-config", "not-json");
    expect(getApiConfig()).toEqual({ baseUrl: "", token: "" });
  });
});

describe("apiRequest", () => {
  beforeEach(() => {
    localStorage.setItem(
      "mihomo-api-config",
      JSON.stringify({ baseUrl: "http://localhost:8051", token: "my-token" })
    );
  });

  afterEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
  });

  it("returns data on successful response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      statusText: "OK",
      text: async () => JSON.stringify({ status: "success", data: { count: 42 }, message: null }),
      json: async () => ({ status: "success", data: { count: 42 }, message: null }),
    } as unknown as Response);

    const result = await apiRequest<{ count: number }>("/test");
    expect(result).toEqual({ count: 42 });
    expect(global.fetch).toHaveBeenCalledWith(
      "http://localhost:8051/api/v1/test",
      expect.objectContaining({
        headers: expect.objectContaining({
          "Content-Type": "application/json",
          Authorization: "Bearer my-token",
        }),
      })
    );
  });

  it("throws ApiError when response is not ok", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 500,
      statusText: "Internal Server Error",
      text: async () => JSON.stringify({ status: "error", data: null, message: "server error" }),
      json: async () => ({ status: "error", data: null, message: "server error" }),
    } as unknown as Response);

    await expect(apiRequest("/test")).rejects.toThrow(ApiError);
    await expect(apiRequest("/test")).rejects.toThrow("server error");
  });

  it("throws ApiError when response ok but body status is error", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => JSON.stringify({ status: "error", data: null, message: "business error" }),
      json: async () => ({ status: "error", data: null, message: "business error" }),
    } as unknown as Response);

    await expect(apiRequest("/test")).rejects.toThrow(ApiError);
    await expect(apiRequest("/test")).rejects.toThrow("business error");
  });

  it("throws ApiError on invalid JSON response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: "OK",
      text: async () => "not-json",
    } as unknown as Response);

    await expect(apiRequest("/test")).rejects.toThrow(ApiError);
    await expect(apiRequest("/test")).rejects.toThrow("无效响应");
  });

  it("throws ApiError when baseUrl is not configured", async () => {
    localStorage.setItem("mihomo-api-config", JSON.stringify({ baseUrl: "", token: "" }));
    await expect(apiRequest("/test")).rejects.toThrow("API 地址未配置");
  });

  it("throws ApiError on network failure", async () => {
    global.fetch = vi.fn().mockRejectedValue(new Error("net::ERR_FAILED"));
    await expect(apiRequest("/test")).rejects.toThrow("网络错误: net::ERR_FAILED");
  });
});

describe("checkHealth", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("returns true for ok response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: true,
    } as unknown as Response);
    const result = await checkHealth("http://localhost:8051");
    expect(result).toBe(true);
  });

  it("returns false for non-ok response", async () => {
    global.fetch = vi.fn().mockResolvedValue({
      ok: false,
    } as unknown as Response);
    const result = await checkHealth("http://localhost:8051");
    expect(result).toBe(false);
  });

  it("returns false on network error", async () => {
    global.fetch = vi.fn().mockRejectedValue(new Error("fail"));
    const result = await checkHealth("http://localhost:8051");
    expect(result).toBe(false);
  });
});

describe("logStreamSocket", () => {
  type MockSocket = {
    close: ReturnType<typeof vi.fn>;
    send: ReturnType<typeof vi.fn>;
    readyState: number;
    onopen?: () => void;
    onmessage?: (event: { data: unknown }) => void;
    onerror?: (err: unknown) => void;
    onclose?: () => void;
  };

  let sockets: MockSocket[];
  let WebSocketSpy: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    sockets = [];
    WebSocketSpy = vi.fn().mockImplementation(function () {
      const sock: MockSocket = {
        close: vi.fn(),
        send: vi.fn(),
        readyState: 0,
      };
      sockets.push(sock);
      return sock;
    });
    (global as unknown as { WebSocket: typeof WebSocketSpy }).WebSocket = WebSocketSpy;
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("ignores non-text messages with a warning", () => {
    const consoleWarnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    logStreamSocket("http://localhost:8051", "token", vi.fn());
    const sock = sockets[0];
    sock.onopen?.();
    sock.onmessage?.({ data: new Blob(["hello"]) });
    expect(consoleWarnSpy).toHaveBeenCalledWith(
      "收到非文本 WebSocket 消息，已忽略 (类型:",
      "object",
      ")"
    );
  });

  it("closes socket when onMessage callback throws", () => {
    const onMessage = vi.fn().mockImplementation(() => {
      throw new Error("callback failure");
    });
    logStreamSocket("http://localhost:8051", "token", onMessage);
    const sock = sockets[0];
    sock.onopen?.();
    sock.onmessage?.({
      data: JSON.stringify({ type: "system", timestamp: "2024-01-01T00:00:00Z" }),
    });
    expect(onMessage).toHaveBeenCalled();
    expect(sock.close).toHaveBeenCalledWith(1011, "message-handler-error");
  });

  it("reconnects up to max attempts then stops", () => {
    logStreamSocket("http://localhost:8051", "token", vi.fn());
    // Simulate repeated close events to trigger reconnect
    for (let i = 0; i < 15; i++) {
      const sock = sockets[sockets.length - 1];
      sock.onclose?.();
      vi.runOnlyPendingTimers();
    }
    // Initial connect + 9 successful reconnects; the 10th schedule triggers connect() which returns early
    expect(WebSocketSpy).toHaveBeenCalledTimes(10);
  });

  it("stops reconnecting after unsubscribe is called", () => {
    const unsubscribe = logStreamSocket("http://localhost:8051", "token", vi.fn());
    unsubscribe();
    const sock = sockets[0];
    sock.onclose?.();
    vi.runAllTimers();
    expect(WebSocketSpy).toHaveBeenCalledTimes(1);
  });

  it("uses wss protocol for https baseUrl", () => {
    logStreamSocket("https://localhost:8051", "token", vi.fn());
    expect(WebSocketSpy).toHaveBeenCalledWith("wss://localhost:8051/ws/logs?token=token");
  });

  it("URL-encodes token in WebSocket URL", () => {
    logStreamSocket("http://localhost:8051", "token&value=1", vi.fn());
    expect(WebSocketSpy).toHaveBeenCalledWith(
      "ws://localhost:8051/ws/logs?token=token%26value%3D1"
    );
  });
});
