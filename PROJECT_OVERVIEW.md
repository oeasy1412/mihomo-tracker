# Mihomo Connections Tracker 项目总览（代码对齐版）

## 1. 项目定位

`mihomo-tracker` 用于采集 Mihomo 连接快照、汇聚多 Agent 数据、并提供统计分析与日志可视化。

核心能力：

- Master-Agent 架构
- Agent 离线缓存与恢复后补同步
- 连接快照统计查询
- 关闭连接审计日志
- WebSocket 实时日志流
- Next.js 前端监控面板

## 2. 技术栈

- **后端**：Rust + Tokio + Warp + sqlx + SQLite
- **前端**：Next.js 14 (App Router) + React 18 + TypeScript + Shadcn UI (Tailwind CSS) + Recharts + react-virtuoso
- **可观测性**：`tracing` + `tracing-subscriber` + `tracing-appender` 结构化日志、异步 Span 上下文、非阻塞文件落盘与 WebSocket 实时推送

完整选型说明见 [ARCHITECTURE.md](./ARCHITECTURE.md)。

## 3. 目录结构

```text
backend/                     Rust 后端（master / agent）
  src/                       源代码（main.rs, api.rs, db.rs, agent.rs, master.rs 等）
  tests/                     集成测试
  run_master.bat             本地 Master 启动脚本
  run_agent.bat              本地 Agent 启动脚本
frontend/                    Next.js 14 前端
  app/                       App Router 页面与布局
  components/                通用 UI 组件（排序表头、分页、空状态等）
  hooks/                     自定义 React Hooks
  lib/                       API 客户端与工具函数
  types/                     TypeScript 类型定义
.claude/skills/              项目专属 Claude Skills（speckit 系列）
specs/                       需求与任务文档
api.md                       API 文档（与代码对齐）
ARCHITECTURE.md              架构说明（当前实现）
AGENTS.md                    开发指南
```

## 4. 后端运行模式

### 4.1 Master

职责：

- 接收 Agent 同步
- 提供 REST 查询接口
- 推送 `WS /ws/logs`
- 可选直连本地 Mihomo（独立采集模式）
- 定时清理过期审计日志（`--log-retention-days`）

### 4.2 Agent

职责：

- 订阅 Mihomo `/connections` WebSocket
- 本地落库 `agent.db`
- 实时上报关闭连接日志
- 定时批量同步到 Master
- Agent在Master离线时在本地缓存数据，等Master恢复后自动同步（有超时删除机制）

## 5. 数据存储与日志文件

### 5.1 SQLite 数据库

- Master 默认: `master.db`
- Agent 默认: `agent.db`
- 路由器场景推荐: `/tmp/mihomo-tracker/agent.db`（内存盘，保护闪存寿命）

关键表：

- `connections`: 连接快照
- `sync_state`: Agent 同步状态（记录已同步到 Master 的水位线）
- `connection_logs`: 已关闭连接的审计日志（关闭后才会同步）

### 5.2 文件日志（tracing-appender）

`tracing-appender` 负责将 `tracing` 事件异步写入磁盘：

- **目录**: `./logs`（可通过 `--log-dir` 修改）
- **格式**: `app.log` + 按天轮转 `app.log.YYYY-MM-DD`
- **保留**: 启动时自动清理超过 7 天的旧日志文件
- **特点**: 非阻塞写入，避免高并发时日志 IO 阻塞 Tokio 任务

### 5.3 数据保留策略

| 层级 | 表 | 触发时机 | 规则 |
|------|-----|---------|------|
| **Agent** | `connections` | 连接关闭时立即删除；兜底每小时 | 删除 `last_updated` 超期（`--data-retention-days`，默认 1 天） |
| **Agent** | `connection_logs` | 每小时 | 已同步 (`synced=1`) 立即删除；未同步 (`synced=0`) 超 `--log-retention-days`（默认 7 天）删除 |
| **Master** | `connections` | 每小时 | 删除 `last_updated` 超 1 天未更新（兜底） |
| **Master** | `connection_logs` | 每日定时任务 | `datetime(end) < now - --log-retention-days`（默认 30 天） |

## 6. API 能力

后端已实现接口：

- 健康检查：`GET /api/v1/health`
- 同步：`POST /api/v1/sync`
- 审计同步：`POST /api/v1/logs/connections/sync`
- 关闭连接上报：`POST /api/v1/logs/connection-closed`
- 统计：`GET /api/v1/stats`
- 连接查询：`GET /api/v1/connections`
- 审计日志：`GET /api/v1/logs/connections`
- Agent 列表：`GET /api/v1/agents`
- Agent 状态：`GET /api/v1/agents/{id}/status`
- 筛选选项：`GET /api/v1/filter-options`
- 实时日志：`WS /ws/logs`

详见 [api.md](./api.md)。

## 7. 前端页面

主入口：`frontend/app/dashboard`

当前页面：

- `/dashboard` 概览页
- `/dashboard/connections` 活跃连接页
- `/dashboard/audit` IP流量审计页
- `/dashboard/logs` 日志中心页
- `/dashboard/agents` 代理状态页

前端通过 `frontend/lib/api.ts` 统一调用后端，并在浏览器 `localStorage` 中保存 API 地址与 Token（`mihomo-api-config`）。

## 8. 开发命令

### 8.1 Backend

```bash
cd backend
cargo build --release
cargo fmt --all
cargo clippy --all
cargo test --all
```

### 8.2 Frontend

```bash
cd frontend
npm install
npm run dev
npm run build
npm run test
npm run lint
npm run format
npm run format:check
```
