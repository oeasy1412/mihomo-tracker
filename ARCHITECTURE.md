# Mihomo Connections Tracker 架构说明（当前实现）

本文档描述仓库当前代码状态，不包含未落地的规划接口或模块。

## 1. 总体架构

系统由两部分组成：

- `backend/` Rust 服务
- `frontend/` Next.js 14 可视化面板

后端通过同一个二进制提供两种运行角色：

- `master`: 中央汇聚与查询节点
- `agent`: 边缘采集与同步节点

## 2. 技术栈选型

### 2.1 后端核心

- **Rust + Tokio**：内存安全、零拷贝网络处理、无需 GC 停顿；Tokio 是异步运行时事实标准，支撑 Mihomo WebSocket 长连接、Agent 批量同步、Master 并发请求。
- **Warp**：基于 Filter 组合式的 Web 框架，原生支持 WebSocket（`warp::ws`），轻量且路由可测试性强。
- **sqlx**：编译期检查 SQL 语法与参数类型，避免 ORM 膨胀，保留手写优化 SQL 的灵活性。
- **tokio-tungstenite**：与 Tokio runtime 深度集成的 WebSocket 实现，Agent 用它订阅 Mihomo，Master 用它推送日志事件。
- **SQLite**：零配置、单文件、随进程启动即可用，非常适合边缘 Agent 和轻量 Master。

### 2.2 日志与可观测性：tracing 家族

我们选择 `tracing` + `tracing-subscriber` + `tracing-appender`，而非传统 `log` + `env_logger`：

1. **结构化日志**：`tracing` 输出的是带键值对的结构化事件，开启 JSON feature 后可直接对接 Loki / ELK。
2. **异步原生上下文（Span）**：在 Tokio 任务频繁跳转的异步环境中，`Span` 能跨任务边界传播上下文，所有子事件自动继承字段（如 `agent_id`），避免并发日志串行混淆。
3. **非阻塞文件落盘**：`tracing-appender` 通过内存 channel 异步刷盘，配合 `rolling::daily` 按天轮转，启动时自动清理超过 7 天的旧日志文件。
4. **统一可观测性入口**：目前做日志，未来若需 OpenTelemetry 分布式链路或指标收集，可直接通过 `tracing-opentelemetry` 扩展。
5. **生态一致性**：`sqlx`、`reqwest`、Tokio 本身的诊断输出都原生兼容 `tracing`。

### 2.3 前端

- **Next.js 14 (App Router)**：React Server Components 减少首屏 JS 体积。
- **Shadcn UI (Tailwind)**：基于 Radix UI 的无头组件库，样式可控，避免体积膨胀。
- **Recharts**：轻量声明式图表库。
- **react-virtuoso**：虚拟滚动处理持续增长的长列表（如实时日志 WebSocket 消息流），保持 60fps 渲染。

## 3. 后端模块划分

核心文件：

- `backend/src/main.rs`: CLI 入口，分发 `master` / `agent`
- `backend/src/config.rs`: 命令行参数定义
- `backend/src/agent.rs`: Agent 采集、实时上报、批量同步、数据清理
- `backend/src/master.rs`: Master API 服务、可选本地采集、日志清理
- `backend/src/api.rs`: Mihomo 客户端、Master 客户端、HTTP/WS 路由
- `backend/src/common.rs`: 数据模型与 `process_connections` 核心处理逻辑
- `backend/src/db.rs`: SQLite schema 与查询/写入实现
- `backend/src/logger.rs`: tracing 初始化、文件日志、WebSocket 日志广播

集成测试：

- `backend/tests/`: API 路由、数据库查询、数据模型等集成测试

## 4. 数据流

### 4.1 Agent 侧

1. 连接 Mihomo WebSocket `ws://<mihomo>/connections?token=...`
2. 接收全量连接快照并调用 `process_connections`
3. 将变化写入本地 `agent.db`（`connections`）
4. 对已关闭连接：通过 `tokio::sync::mpsc::unbounded_channel` 发送给独立 worker，由 worker 批量落库到 `connection_logs` 并可选实时上报 Master（避免 fire-and-forget 导致错误丢失）
5. 若配置了 Master：
   - 实时调用 `POST /api/v1/logs/connection-closed`
   - 周期批量调用 `POST /api/v1/sync` 和 `POST /api/v1/logs/connections/sync`
   - 同步成功后以**本批次最后一条记录的 `last_updated`** 更新 `sync_state` 水位线

### 4.2 Master 侧

1. 接收 Agent 同步请求并写入 `master.db`
2. 提供统计、连接、审计、筛选、Agent 状态 API
3. 通过 `WS /ws/logs` 推送：
- 系统日志（`system`）
- 关闭连接事件（`connection_closed`）
4. 可选直接连接本地 Mihomo（独立运行模式）

## 5. 存储模型与日志机制

### 5.1 SQLite 表

- `connections`: 连接快照主表，主键 `(id, agent_id)`
- `sync_state`: Agent 本地同步状态
- `connection_logs`: 关闭连接审计日志，主键 `(id, agent_id)`

### 5.2 文件日志（tracing-appender）

`tracing-appender` 负责将 `tracing` 事件异步写入磁盘：
- 目录: `./logs`（可通过 `--log-dir` 修改）
- 格式: `app.log` + 按天轮转 `app.log.YYYY-MM-DD`
- 保留: 启动时自动清理超过 7 天的旧日志文件
- 特点: 非阻塞写入，避免高并发时日志 IO 阻塞 Tokio 任务

### 5.3 数据清理策略

- **Agent `connections`**：连接关闭后立即删除；兜底清理超过 `--data-retention-days`（默认 1 天）未更新的记录
- **Agent `connection_logs`**：已同步记录每小时立即清理；未同步记录超过 `--log-retention-days`（默认 7 天）强制清理；每次清理后执行 `VACUUM` 回收磁盘空间
- **Master `connections`**：兜底清理超过 1 天未更新的记录（精确活跃连接由全量同步保证）
- **Master `connection_logs`**：清理超期记录（`--log-retention-days`，默认 30 天）

## 6. API 与实时通道

当前已实现接口：

- `GET /api/v1/health`
- `POST /api/v1/sync`
- `POST /api/v1/logs/connections/sync`
- `POST /api/v1/logs/connection-closed`
- `GET /api/v1/stats`
- `GET /api/v1/connections`
- `GET /api/v1/logs/connections`
- `GET /api/v1/agents`
- `GET /api/v1/agents/{id}/status`
- `GET /api/v1/filter-options`
- `WS /ws/logs`

详情见 [api.md](./api.md)。

## 7. 前端结构

前端使用 Next.js 14 App Router，核心目录如下：

### 7.1 页面

- `frontend/app/dashboard/page.tsx`: 概览页（统计卡片 + 时间序列趋势图 + 分组统计）
- `frontend/app/dashboard/connections/page.tsx`: 活跃连接页（支持筛选、排序、分页）
- `frontend/app/dashboard/audit/page.tsx`: **IP 流量审计页（三层钻取）**
  - 第一层：按 **源 IP** 分组，点击下钻
  - 第二层：按 **Rule 链路** 分组，点击下钻
  - 第三层：按 **目标域名 / IP : 端口** 展示明细
  - 每层均支持面包屑返回、柱状图联动下钻、多维度排序（count / download / upload / total）
  - 支持“排除 DIRECT”快捷筛选
  - 第二、三层底部展示对应筛选条件的流量趋势图
- `frontend/app/dashboard/logs/page.tsx`: 日志中心（WebSocket 实时日志流，支持按级别/类型过滤）
- `frontend/app/dashboard/agents/page.tsx`: Agent 状态视图（列表 + 单 Agent 详情弹窗）

### 7.2 共享组件

- `frontend/app/dashboard/_components/`
  - `stat-cards.tsx`: 汇总统计卡片
  - `time-series-chart.tsx`: 时间序列趋势图（支持 `minute | hour | day | week | month` 粒度）
  - `grouped-stats.tsx`: 分组统计表格与图表
  - `filters-popover.tsx`: 统一筛选浮层
- `frontend/app/dashboard/logs/_components/`
  - `log-stream.tsx`: WebSocket 实时日志虚拟滚动列表（`react-virtuoso`）
  - `log-filters.tsx`: 日志级别与类型过滤
- `frontend/app/dashboard/agents/_components/`
  - `agent-detail.tsx`: Agent 详情面板

### 7.3 通用组件与 Hooks

- `frontend/components/`: 跨页面通用组件（`sortable-table-head`、`table-pagination-footer`、`page-header`、`settings-dialog`、`theme-toggle` 等）
- `frontend/hooks/`: `use-api-polling`、`use-polling`、`use-api-config`
- `frontend/lib/api.ts`: 统一 API 客户端，所有 `/api/v1/*` 调用入口
- `frontend/types/api.ts`: 全站 TypeScript 类型定义

## 8. 部署与交叉编译

### 8.1 Master-Agent 分离部署

生产环境推荐将 Agent 部署在边缘路由器（如运行 ImmortalWrt 24.10+ 的 x86_64 设备），Master 与前端部署在算力更充裕的本地电脑或服务器。Agent 通过 HTTP 将审计日志和连接快照同步到 Master，Master 提供 REST 查询和 WebSocket 实时推送。

### 8.2 musl 目标与 Rustls 选型

为支持 OpenWrt / ImmortalWrt 等基于 musl libc 的系统，后端已统一使用 Rustls：
- `sqlx` 启用 `runtime-tokio-rustls`
- `reqwest` 启用 `rustls-tls`

优点：
- 纯 Rust 实现，交叉编译到 `x86_64-unknown-linux-musl` 零痛苦。
- 生成的单二进制文件没有任何外部 TLS 依赖，OpenWrt 上无需安装 `libopenssl`。
- 相比 native-tls，体积仅增加约数百 KB。

### 8.3 闪存友好的存储策略

路由器闪存有写入寿命限制。建议将 Agent 的 SQLite 数据库和文件日志目录指向 `/tmp/`（内存盘）：

```bash
agent --local-database /tmp/mihomo-tracker/agent.db --log-dir /tmp/mihomo-tracker/logs
```

- 连接快照（`connections`）和本地审计日志（`connection_logs`）在同步到 Master 后价值降低，允许路由器重启后丢失。
- 实时关闭连接日志已先通过 HTTP 上报 Master，再写入本地 `connection_logs`，最大程度减少单点数据丢失风险。

### 8.4 Master 离线容错与恢复

当连接关闭时，Agent 会先将审计日志批量写入本地 SQLite，然后尝试逐条实时上报到 Master（每条等待完成后才处理下一条）。若 Master 不可达，失败仅记录一条调试日志，不回滚已写入本地数据库的数据。

在定时批量同步阶段，Agent 每次都会先请求 Master 的健康检查接口。若检测到 Master 离线，本轮同步直接跳过，本地未同步的审计日志继续保留。Agent 每小时执行一次四步清理流程：① 清理 connections 超期数据（`--data-retention-days`，默认 1 天）；② 立即清理已同步的 connection_logs；③ 强制清理超期未同步的 connection_logs（`--log-retention-days`，默认 7 天）；④ VACUUM 回收磁盘空间。这意味着 Master 离线不超过 7 天，审计数据不会丢失。

当 Master 恢复在线后，Agent 在下一个同步周期会自动检测到可用状态，并将积压的审计日志批量补传到 Master。`connections` 已改为实时活跃连接（关闭即删），同步时发送当前全量活跃连接快照，Master 收到后做全量替换，确保 Master 侧的活跃连接视图始终精确。整个过程中无需人工干预。

## 9. 运行关系图

```text
Mihomo WS -> Agent(process_connections) -> agent.db
                |                |
                |                +-> connection_logs
                |
                +-> HTTP sync/report -> Master API -> master.db
                                               |
                                               +-> REST 查询
                                               +-> WS /ws/logs 推送
```
