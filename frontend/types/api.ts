export interface ApiConfig {
  baseUrl: string;
  token: string;
}

export interface FilterCriteria {
  from: string | null;
  to: string | null;
  agentId: string | null;
  network: string | null;
  rule: string | null;
  process: string | null;
  source: string | null;
  destination: string | null;
  host: string | null;
  chains: string | null;
  destination_port: string | null;
  exclude_rule: string | null;
}

export interface ConnectionRecord {
  id: string;
  agent_id: string | null;
  download: number;
  upload: number;
  last_updated: string;
  start: string;
  network: string;
  source_ip: string;
  destination_ip: string;
  source_port: string;
  destination_port: string;
  host: string;
  process: string;
  process_path: string;
  special_rules: string;
  chains: string;
  rule: string;
  rule_payload: string;
}

export interface AgentNode {
  id: string;
  last_active: string;
  connections_count: number;
  total_download: number;
  total_upload: number;
  total_traffic: number;
  status: "active" | "inactive" | "unknown";
  networks?: Array<{ network: string; count: number }>;
  rules?: Array<{ rule: string; count: number }>;
}

export interface AgentStatus {
  id: string;
  last_active: string;
  connections_count: number;
  total_download: number;
  total_upload: number;
  total_traffic: number;
  is_active: boolean;
  status: string;
  networks: Array<{ network: string; count: number }>;
  rules: Array<{ rule: string; count: number }>;
}

export interface StatsSummary {
  count: number;
  download: number;
  upload: number;
  total: number;
}

export type GroupByDimension =
  | "network"
  | "rule"
  | "process"
  | "destination"
  | "host"
  | "chains"
  | "node"
  | "source"
  | "destination_port";

export interface GroupedStatItem {
  count: number;
  download: number;
  upload: number;
  total: number;
  source_ip?: string;
  destination_ip?: string;
  host_display?: string;
  host?: string;
  network?: string;
  process?: string;
  node?: string;
  rule?: string;
  chains?: string;
  destination_port?: string;
}

export interface TimeSeriesPoint {
  time: string;
  value: number;
}

export interface ApiResponse<T> {
  status: "success" | "error";
  data: T;
  message: string | null;
}

export interface FilterOption {
  value: string;
  label: string;
  field: string;
}

export interface ConnectionLog {
  id: string;
  agent_id: string;
  source_ip: string;
  destination_ip: string;
  source_port: string;
  destination_port: string;
  host: string;
  rule: string;
  rule_payload: string;
  chains: string;
  network: string;
  process: string;
  process_path: string;
  download: number;
  upload: number;
  start: string;
  end: string;
  special_rules: string;
  synced?: number;
}

export interface ConnectionLogPage {
  total: number;
  items: ConnectionLog[];
}

export type LogLevel = "INFO" | "WARN" | "ERROR";

export interface LogStreamEvent {
  type: "system" | "connection_closed";
  timestamp: string;
  level?: LogLevel;
  target?: string;
  message?: string;
  connection?: ConnectionLog;
}
