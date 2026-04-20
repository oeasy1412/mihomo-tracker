# Mihomo Connections Tracker API 文档（代码对齐版）

本文档基于 `backend/src/api.rs`、`backend/src/common.rs`、`backend/src/db.rs` 当前实现整理。

## 1. 基础约定

- Base URL: `http://<master-host>:<master-port>/api/v1`
- 认证方式: Bearer Token（仅当 Master 启动时配置了 `--api-token`）
- 请求头: `Authorization: Bearer <token>`
- 内容类型: `application/json`

统一响应结构（除 `GET /health`）：

```json
{
  "status": "success | error",
  "data": {},
  "message": null
}
```

错误时：

```json
{
  "status": "error",
  "data": null,
  "message": "错误信息"
}
```

常见状态码：

- `400`: 参数错误
- `401`: 认证失败
- `404`: 路由不存在
- `500`: 服务端或数据库错误

## 2. 健康检查

### `GET /api/v1/health`

- 认证: 不需要
- 返回: 纯文本 `OK`（不是 JSON）

## 3. Agent 同步接口

### `POST /api/v1/sync`

- 认证: 需要（若启用 token）
- 用途: Agent 批量同步 `connections`

请求体：

```json
{
  "agent_id": "agent-1",
  "connections": [
    {
      "id": "conn-1",
      "download": 1024,
      "upload": 512,
      "last_updated": "2026-04-16T12:00:00Z",
      "start": "2026-04-16T11:59:00Z",
      "network": "tcp",
      "conn_type": "http",
      "source_ip": "192.168.1.10",
      "destination_ip": "1.1.1.1",
      "source_geoip": "{}",
      "destination_geoip": "{}",
      "source_ip_asn": "",
      "destination_ip_asn": "",
      "source_port": "12345",
      "destination_port": "443",
      "inbound_ip": "",
      "inbound_port": "",
      "inbound_name": "",
      "inbound_user": "",
      "host": "one.one.one.one",
      "dns_mode": "",
      "uid": 0,
      "process": "chrome.exe",
      "process_path": "",
      "special_proxy": "",
      "special_rules": "",
      "remote_destination": "",
      "dscp": 0,
      "sniff_host": "",
      "chains": "[]",
      "rule": "DIRECT",
      "rule_payload": "",
      "agent_id": "agent-1"
    }
  ],
  "timestamp": "2026-04-16T12:00:01Z"
}
```

成功响应：

```json
{
  "status": "success",
  "data": {
    "message": "数据同步成功",
    "count": 1
  },
  "message": null
}
```

### `POST /api/v1/logs/connections/sync`

- 认证: 需要（若启用 token）
- 用途: Agent 批量同步关闭连接审计日志

请求体：

```json
{
  "agent_id": "agent-1",
  "logs": [
    {
      "id": "conn-1",
      "agent_id": "agent-1",
      "source_ip": "192.168.1.10",
      "destination_ip": "1.1.1.1",
      "host": "one.one.one.one",
      "rule": "DIRECT",
      "chains": "[]",
      "network": "tcp",
      "process": "chrome.exe",
      "download": 1024,
      "upload": 512,
      "start": "2026-04-16T11:59:00Z",
      "end": "2026-04-16T12:00:00Z"
    }
  ]
}
```

成功响应：

```json
{
  "status": "success",
  "data": {
    "message": "connection_logs 同步成功",
    "count": 1
  },
  "message": null
}
```

### `POST /api/v1/logs/connection-closed`

- 认证: 需要（若启用 token）
- 用途: Agent 实时上报单条关闭连接日志
- 请求体: 单个 `ConnectionLog` 对象（同上）

成功响应：

```json
{
  "status": "success",
  "data": {
    "message": "ok"
  },
  "message": null
}
```

## 4. 统计与查询接口

### `GET /api/v1/stats`

- 认证: 需要（若启用 token）
- 统一入口，`type` 必填

公共筛选参数：

- `from` / `to`（ISO 8601）
- `agent_id`
- `network`
- `rule`
- `process`（空字符串或 `"进程为空"` 会被特殊处理为空进程查询）
- `source`
- `destination`
- `host`
- `chains`（模糊匹配）
- `geoip`
- `destination_port`
- `exclude_rule`（排除指定 rule，如 `DIRECT`）

#### 4.1 汇总统计

- 请求: `GET /api/v1/stats?type=summary`
- 返回字段: `count`、`download`、`upload`、`total`

#### 4.2 分组统计

- 请求: `GET /api/v1/stats?type=group&group_by=network`
- `group_by` 支持:
- `network`
- `rule`
- `process`
- `destination`
- `source`
- `host`
- `chains`
- `geoip`
- `destination_port`

排序参数：

- `sort_by`: `count | download | upload | total`（默认 `count`）
- `sort_order`: `asc | desc`（默认 `desc`）
- `limit`: 可选

#### 4.3 时间序列

- 请求: `GET /api/v1/stats?type=timeseries&from=...&to=...`
- `from` 和 `to` 都是必填
- `interval`: `minute | hour | day | week | month`（默认 `day`）
- `metric`: `connections | download | upload | total`（默认 `connections`）

返回字段：

```json
{
  "time": "2026-04-16 12:00:00",
  "value": 12345
}
```

### `GET /api/v1/connections`

- 认证: 需要（若启用 token）
- 用途: 查询当前 `connections` 表数据（活跃快照）

查询参数：

- `agent_id`
- `network`
- `rule`
- `process`（空字符串或 `"进程为空"` 会匹配 `process = '' OR process IS NULL`）
- `source`
- `destination`
- `host`（模糊匹配）
- `chains`（模糊匹配）
- `geoip`（模糊匹配）
- `destination_port`
- `from`（按 `start >= from`）
- `to`（按 `start <= to`）
- `exclude_rule`（排除指定 rule）
- `limit`（默认 100）
- `offset`（仅在传入 `limit` 时生效）
- `sort_by`: `download | upload | start | last_updated`（默认 `last_updated`）；非法值返回 `400 BadRequest`
- `sort_order`: `asc | desc`（默认 `desc`）

返回：`data` 是数组，不包含 `total_count`。

### `GET /api/v1/logs/connections`

- 认证: 需要（若启用 token）
- 用途: 查询历史关闭连接审计日志

查询参数：

- `agent_id`
- `from`（按 `end >= from`）
- `to`（按 `end <= to`）
- `host`（模糊匹配）
- `rule`（精确匹配）
- `network`（精确匹配）
- `keyword`（模糊匹配 `host` / `rule` / `process` / `destination_ip` / `source_ip` / `chains`）
- `sort_by`: `end | download | upload | total`（默认 `end`）
- `sort_order`: `asc | desc`（默认 `desc`）
- `limit`: 默认 20，范围 1-100
- `offset`: 默认 0

返回示例：

```json
{
  "status": "success",
  "data": {
    "total": 128,
    "items": [
      {
        "id": "conn-1",
        "agent_id": "agent-1",
        "source_ip": "192.168.1.10",
        "destination_ip": "1.1.1.1",
        "host": "one.one.one.one",
        "rule": "DIRECT",
        "chains": "[]",
        "network": "tcp",
        "process": "chrome.exe",
        "download": 1024,
        "upload": 512,
        "start": "2026-04-16T11:59:00Z",
        "end": "2026-04-16T12:00:00Z",
        "synced": 1
      }
    ]
  },
  "message": null
}
```

## 5. Agent 状态接口

### `GET /api/v1/agents`

- 认证: 需要（若启用 token）
- 查询参数：
  - `exclude_rule`（可选，排除指定 rule 后再统计连接数与流量）
- 返回字段（每个 agent）：
  - `id`
  - `last_active`
  - `connections_count`
  - `total_download`
  - `total_upload`
  - `total_traffic`
  - `status`（当前实现固定 `unknown`）

### `GET /api/v1/agents/{agent_id}/status`

- 认证: 需要（若启用 token）
- 查询参数：
  - `exclude_rule`（可选，排除指定 rule 后再统计）
- 返回单个 agent 详情：
  - `id`
  - `last_active`
  - `connections_count`
  - `total_download`
  - `total_upload`
  - `total_traffic`
  - `is_active`（最后活跃时间 10 分钟内）
  - `status`（`active | inactive`）
  - `networks`（分组计数）
  - `rules`（分组计数）

## 6. 筛选选项接口

### `GET /api/v1/filter-options`

- 认证: 需要（若启用 token）

查询参数：

- `filter_type`（必填）:
  - `agent_id`
  - `network`
  - `rule`
  - `process`
  - `destination`
  - `host`
  - `geoip`
  - `destination_port`
- `query`（可选，关键字）
- `limit`（可选）

返回示例：

```json
{
  "status": "success",
  "data": [
    {
      "value": "DIRECT",
      "label": "DIRECT",
      "field": "rule"
    }
  ],
  "message": null
}
```

## 7. 实时日志 WebSocket

### `WS /ws/logs`

- 认证: 若启用 token，可通过任一方式：
  - Header: `Authorization: Bearer <token>`
  - Query: `?token=<token>`

实现技术：Master 内部通过 `tokio::sync::broadcast` 维护内存事件总线，`tokio-tungstenite` 将事件推送到所有已连接的 WebSocket 客户端。前端使用 `react-virtuoso` 做虚拟滚动渲染，保证高吞吐量下仍保持 60fps。

当客户端处理速度过慢导致广播 channel lag 时，服务端会发送一条 `gap` 通知后断开连接：

```json
{
  "event_type": "gap",
  "skipped": 123,
  "message": "客户端处理速度过慢，已丢失 123 条事件"
}
```

消息类型：

- `system`

```json
{
  "type": "system",
  "timestamp": "2026-04-16T12:34:56Z",
  "level": "INFO",
  "target": "backend::agent::sync",
  "message": "成功同步 100 条审计日志"
}
```

- `connection_closed`

```json
{
  "type": "connection_closed",
  "timestamp": "2026-04-16T12:35:01Z",
  "connection": {
    "id": "conn-1",
    "agent_id": "agent-1",
    "source_ip": "192.168.1.10",
    "destination_ip": "1.1.1.1",
    "host": "one.one.one.one",
    "rule": "DIRECT",
    "chains": "[]",
    "network": "tcp",
    "process": "chrome.exe",
    "download": 1024,
    "upload": 512,
    "start": "2026-04-16T11:59:00Z",
    "end": "2026-04-16T12:00:00Z"
  }
}
```

## 8. 启动参数（见脚本）
