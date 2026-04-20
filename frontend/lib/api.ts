import type {
  ApiResponse,
  FilterCriteria,
  ConnectionRecord,
  AgentNode,
  AgentStatus,
  StatsSummary,
  GroupedStatItem,
  GroupByDimension,
  TimeSeriesPoint,
  FilterOption,
  ConnectionLogPage,
  LogStreamEvent,
} from "@/types/api";

const REQUEST_TIMEOUT = 5000;
export const API_CONFIG_STORAGE_KEY = "mihomo-api-config";
export const API_CONFIG_UPDATED_EVENT = "mihomo:api-config-updated";

export class ApiError extends Error {
  constructor(
    message: string,
    public status?: number
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function buildQueryString(
  base: string,
  params: Record<string, string | number | undefined>
): string {
  const qs = new URLSearchParams();
  Object.entries(params).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== "") {
      qs.append(key, String(value));
    }
  });
  const query = qs.toString();
  return query ? `${base}?${query}` : base;
}

async function fetchWithTimeout(
  url: string,
  options: RequestInit,
  timeout = REQUEST_TIMEOUT
): Promise<Response> {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), timeout);
  let externalAbortHandler: (() => void) | null = null;

  if (options.signal) {
    if (options.signal.aborted) {
      controller.abort();
    } else {
      const handler = () => controller.abort();
      externalAbortHandler = handler;
      options.signal.addEventListener("abort", handler);
    }
  }
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    return response;
  } finally {
    clearTimeout(id);
    if (externalAbortHandler && options.signal) {
      options.signal.removeEventListener("abort", externalAbortHandler);
    }
  }
}

export async function apiRequest<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const config = getApiConfig();
  if (!config.baseUrl) {
    throw new ApiError("API 地址未配置");
  }

  const url = `${config.baseUrl}/api/v1${path}`;
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
  };
  if (options.headers) {
    if (options.headers instanceof Headers) {
      options.headers.forEach((value, key) => {
        headers[key] = value;
      });
    } else if (Array.isArray(options.headers)) {
      options.headers.forEach(([key, value]) => {
        headers[key] = value;
      });
    } else {
      Object.assign(headers, options.headers);
    }
  }
  if (config.token) {
    headers["Authorization"] = `Bearer ${config.token}`;
  }

  let response: Response;
  try {
    response = await fetchWithTimeout(url, {
      ...options,
      headers,
    });
  } catch (err) {
    if (err instanceof Error && err.name === "AbortError") {
      throw new ApiError("请求超时，请检查网络或后端状态");
    }
    if (err instanceof Error) {
      throw new ApiError(`网络错误: ${err.message}`);
    }
    throw new ApiError("网络错误，无法连接到后端");
  }

  const responseText = await response.text();
  let data: ApiResponse<T>;
  try {
    data = JSON.parse(responseText) as ApiResponse<T>;
  } catch {
    throw new ApiError(
      `无效响应 (${response.status}): ${responseText.slice(0, 500)}${responseText.length > 500 ? "..." : ""}`,
      response.status
    );
  }

  if (!response.ok || data.status === "error") {
    throw new ApiError(data.message || `请求失败: ${response.status}`, response.status);
  }

  return data.data;
}

function isApiConfigLike(obj: unknown): obj is { baseUrl?: string; token?: string } {
  return typeof obj === "object" && obj !== null;
}

export function getApiConfig(): { baseUrl: string; token: string } {
  if (typeof window === "undefined") return { baseUrl: "", token: "" };
  const raw = localStorage.getItem(API_CONFIG_STORAGE_KEY);
  if (!raw) return { baseUrl: "", token: "" };
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!isApiConfigLike(parsed)) {
      return { baseUrl: "", token: "" };
    }
    return {
      baseUrl: typeof parsed.baseUrl === "string" ? parsed.baseUrl : "",
      token: typeof parsed.token === "string" ? parsed.token : "",
    };
  } catch (e) {
    console.warn("读取 API 配置失败，localStorage 内容可能已损坏:", e);
    localStorage.removeItem(API_CONFIG_STORAGE_KEY);
    return { baseUrl: "", token: "" };
  }
}

export function setApiConfig(config: { baseUrl: string; token: string }) {
  if (typeof window !== "undefined") {
    localStorage.setItem(API_CONFIG_STORAGE_KEY, JSON.stringify(config));
    window.dispatchEvent(new Event(API_CONFIG_UPDATED_EVENT));
  }
}

export function buildFilterParams(filters: FilterCriteria): Record<string, string> {
  const params: Record<string, string> = {};
  if (filters.from) params.from = filters.from;
  if (filters.to) params.to = filters.to;
  if (filters.agentId) params.agent_id = filters.agentId;
  if (filters.network) params.network = filters.network;
  if (filters.rule) params.rule = filters.rule;
  if (filters.process) params.process = filters.process;
  if (filters.source) params.source = filters.source;
  if (filters.destination) params.destination = filters.destination;
  if (filters.host) params.host = filters.host;
  if (filters.chains) params.chains = filters.chains;
  if (filters.destination_port) params.destination_port = filters.destination_port;
  if (filters.exclude_rule) params.exclude_rule = filters.exclude_rule;
  return params;
}

// Specific API helpers
export async function fetchStatsSummary(filters: FilterCriteria, signal?: AbortSignal): Promise<StatsSummary> {
  return apiRequest<StatsSummary>(
    buildQueryString("/stats", { type: "summary", ...buildFilterParams(filters) }),
    { signal }
  );
}

export type GroupedStatsSortBy = "total" | "count" | "download" | "upload";

export async function fetchGroupedStats(
  groupBy: GroupByDimension,
  filters: FilterCriteria,
  options: { sortBy?: GroupedStatsSortBy; sortOrder?: "asc" | "desc"; limit?: number } = {},
  signal?: AbortSignal
): Promise<GroupedStatItem[]> {
  const data = await apiRequest<unknown>(
    buildQueryString("/stats", {
      type: "group",
      group_by: groupBy,
      ...buildFilterParams(filters),
      sort_by: options.sortBy || "total",
      sort_order: options.sortOrder || "desc",
      limit: options.limit || 10,
    }),
    { signal }
  );
  if (!Array.isArray(data)) {
    console.error("API response validation failed: expected array for grouped stats, got", typeof data, data);
    throw new ApiError("服务器返回了无效的数据格式 (expected array)");
  }
  return data as GroupedStatItem[];
}

export type TimeSeriesInterval = "hour" | "day" | "week" | "month" | "minute";
export type TimeSeriesMetric = "total" | "download" | "upload" | "connections";

export async function fetchTimeSeriesStats(
  filters: FilterCriteria,
  options: {
    interval?: TimeSeriesInterval;
    metric?: TimeSeriesMetric;
  } = {},
  signal?: AbortSignal
): Promise<TimeSeriesPoint[]> {
  const data = await apiRequest<unknown>(
    buildQueryString("/stats", {
      type: "timeseries",
      interval: options.interval || "hour",
      metric: options.metric || "total",
      ...buildFilterParams(filters),
    }),
    { signal }
  );
  if (!Array.isArray(data)) {
    console.error("API response validation failed: expected array for time series, got", typeof data, data);
    throw new ApiError("服务器返回了无效的数据格式 (expected array)");
  }
  return data as TimeSeriesPoint[];
}

export interface TimezoneInfo {
  timezone: string;
  offset_minutes: number;
}

export async function fetchTimezone(signal?: AbortSignal): Promise<TimezoneInfo> {
  return apiRequest<TimezoneInfo>("/timezone", { signal });
}

export type ConnectionSortColumn = "last_updated" | "download" | "upload" | "total" | "start";
export type ConnectionLogSortColumn = "end" | "download" | "upload" | "total";

export async function fetchConnections(
  filters: FilterCriteria,
  options: {
    limit?: number;
    offset?: number;
    sortBy?: ConnectionSortColumn;
    sortOrder?: "asc" | "desc";
  } = {},
  signal?: AbortSignal
): Promise<ConnectionRecord[]> {
  const data = await apiRequest<unknown>(
    buildQueryString("/connections", {
      ...buildFilterParams(filters),
      limit: options.limit || 100,
      offset: options.offset || 0,
      sort_by: options.sortBy || "last_updated",
      sort_order: options.sortOrder || "desc",
    }),
    { signal }
  );
  if (!Array.isArray(data)) {
    console.error("API response validation failed: expected array for connections, got", typeof data, data);
    throw new ApiError("服务器返回了无效的数据格式 (expected array)");
  }
  return data as ConnectionRecord[];
}

export interface ConnectionLogFilters {
  agentId?: string;
  from?: string;
  to?: string;
  source?: string;
  host?: string;
  rule?: string;
  network?: string;
  keyword?: string;
}

export async function fetchConnectionLogs(
  filters: ConnectionLogFilters = {},
  pagination: {
    limit?: number;
    offset?: number;
    sortBy?: ConnectionLogSortColumn;
    sortOrder?: "asc" | "desc";
  } = {},
  signal?: AbortSignal
): Promise<ConnectionLogPage> {
  return apiRequest<ConnectionLogPage>(
    buildQueryString("/logs/connections", {
      agent_id: filters.agentId,
      from: filters.from,
      to: filters.to,
      source: filters.source,
      host: filters.host,
      rule: filters.rule,
      network: filters.network,
      keyword: filters.keyword,
      limit: pagination.limit ?? 20,
      offset: pagination.offset ?? 0,
      sort_by: pagination.sortBy ?? "end",
      sort_order: pagination.sortOrder ?? "desc",
    }),
    { signal }
  );
}

function toWebSocketUrl(baseUrl: string, token: string): string {
  const url = new URL(baseUrl);
  url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
  url.pathname = "/ws/logs";
  if (token) {
    url.searchParams.set("token", token);
  }
  return url.toString();
}

export function logStreamSocket(
  baseUrl: string,
  token: string,
  onMessage: (event: LogStreamEvent) => void,
  onPermanentClose?: (reason: string) => void
): () => void {
  let socket: WebSocket | null = null;
  let stopped = false;
  let reconnectDelay = 1000;
  const maxReconnectDelay = 30000;
  const maxReconnectAttempts = 10;
  let reconnectAttempts = 0;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  const connect = () => {
    if (stopped || reconnectAttempts >= maxReconnectAttempts) {
      if (reconnectAttempts >= maxReconnectAttempts) {
        console.error("WebSocket 达到最大重连次数，停止重连");
        onPermanentClose?.("max-reconnect-exceeded");
      }
      return;
    }

    try {
      socket = new WebSocket(toWebSocketUrl(baseUrl, token));
    } catch (err) {
      console.error("WebSocket 创建失败:", err);
      scheduleReconnect();
      return;
    }

    socket.onopen = () => {
      reconnectDelay = 1000;
      reconnectAttempts = 0;
    };

    socket.onmessage = (event) => {
      if (typeof event.data !== "string") {
        console.warn("收到非文本 WebSocket 消息，已忽略 (类型:", typeof event.data, ")");
        return;
      }
      let parsed: LogStreamEvent;
      try {
        parsed = JSON.parse(event.data) as LogStreamEvent;
      } catch (err) {
        console.warn("收到无法解析的 WebSocket 消息:", event.data, err);
        return;
      }
      try {
        onMessage(parsed);
      } catch (err) {
        console.error("WebSocket 消息处理回调异常，关闭连接强制重连:", err);
        socket?.close(1011, "message-handler-error");
      }
    };

    socket.onerror = (err) => {
      console.error("WebSocket 错误:", err);
      socket?.close();
    };

    socket.onclose = () => {
      if (!stopped) scheduleReconnect();
    };
  };

  const scheduleReconnect = () => {
    if (stopped) return;
    reconnectAttempts += 1;
    reconnectTimer = setTimeout(() => {
      if (!stopped) connect();
    }, reconnectDelay);
    reconnectDelay = Math.min(reconnectDelay * 2, maxReconnectDelay);
  };

  connect();

  return () => {
    stopped = true;
    if (reconnectTimer) clearTimeout(reconnectTimer);
    if (socket && socket.readyState !== WebSocket.CLOSED) {
      socket.close(1000, "manual-close");
    }
  };
}

export async function fetchAgents(excludeRule?: string, signal?: AbortSignal): Promise<AgentNode[]> {
  const data = await apiRequest<unknown>(
    buildQueryString("/agents", { exclude_rule: excludeRule }),
    { signal }
  );
  if (!Array.isArray(data)) {
    console.error("API response validation failed: expected array for agents, got", typeof data, data);
    throw new ApiError("服务器返回了无效的数据格式 (expected array)");
  }
  return data as AgentNode[];
}

export async function fetchAgentStatus(agentId: string, excludeRule?: string, signal?: AbortSignal): Promise<AgentStatus> {
  return apiRequest<AgentStatus>(
    buildQueryString(`/agents/${encodeURIComponent(agentId)}/status`, { exclude_rule: excludeRule }),
    { signal }
  );
}

export async function fetchFilterOptions(filterType: string, query?: string, limit = 50, signal?: AbortSignal): Promise<FilterOption[]> {
  const data = await apiRequest<unknown>(
    buildQueryString("/filter-options", {
      filter_type: filterType,
      query,
      limit,
    }),
    { signal }
  );
  if (!Array.isArray(data)) {
    console.error("API response validation failed: expected array for filter options, got", typeof data, data);
    throw new ApiError("服务器返回了无效的数据格式 (expected array)");
  }
  return data as FilterOption[];
}

export async function checkHealth(baseUrl: string): Promise<boolean> {
  const controller = new AbortController();
  const id = setTimeout(() => controller.abort(), REQUEST_TIMEOUT);
  try {
    const res = await fetch(`${baseUrl}/api/v1/health`, {
      signal: controller.signal,
    });
    return res.ok;
  } catch (err) {
    console.warn("Health check failed:", err);
    return false;
  } finally {
    clearTimeout(id);
  }
}
