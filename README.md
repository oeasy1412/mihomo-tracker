# Mihomo Connections Tracker

基于 Rust + Next.js 的 Mihomo 连接监控系统，采用 Master-Agent 架构。

## 核心功能

- 多 Agent 采集 Mihomo 连接快照
- Agent 离线缓存与恢复后补同步
- Master 统一统计查询
- 关闭连接审计日志
- WebSocket 实时日志推送
- 前端仪表盘可视化

## 技术栈

Rust + Tokio + Warp + sqlx + SQLite 后端；Next.js 14 + Shadcn UI + Recharts 前端；`tracing` 生态负责结构化日志、异步 Span 上下文与非阻塞文件落盘。完整选型说明见 [ARCHITECTURE.md](./ARCHITECTURE.md)。

## 仓库结构

```text
backend/      Rust 后端（master / agent）
frontend/     Next.js 14 前端
specs/        需求与任务文档
api.md        API 说明（代码对齐）
```

## 快速开始

### 1. 启动 Master

```bash
cd backend
./run_master.bat
# 或者使用 cargo
cargo run --release -- master \
  --database ../master.db \
  --log-dir ./logs \
  --log-retention-days 30 \
  --listen-host 0.0.0.0 \
  --listen-port 8051 \
  --api-token YOUR_MASTER_TOKEN
```

### 2. 启动 Agent

```bash
cd backend
./run_agent.bat
# 或者使用 cargo
cargo run --release -- agent \
  --local-database ../agent.db \
  --log-dir ./logs \
  --master-url http://127.0.0.1:8051 \
  --master-token YOUR_MASTER_TOKEN \
  --agent-id agent-1 \
  --mihomo-host 127.0.0.1 \
  --mihomo-port 9097 \
  --mihomo-token YOUR_MIHOMO_TOKEN
```

### 3. 启动前端

```bash
cd frontend
npm install
npm run dev
```

访问 `http://localhost:3000`，在设置中配置后端地址和 token。

## 部署场景

### 本地全栈（开发/测试）

Master、Agent、前端均运行在同一台机器，参考上方[快速开始](#快速开始)。

### 边缘路由 + 本地 Master（推荐生产场景）

- **Agent**：运行在 OpenWrt / ImmortalWrt 等边缘路由器（典型如 x86_64 / musl libc，例如 Intel Celeron J1900）。
- **Master + 前端**：运行在本地电脑或服务器，负责统一汇聚、查询与可视化。

后端已采用 **Rustls**（`sqlx` 与 `reqwest` 均使用 rustls），交叉编译到 musl 目标无需系统 OpenSSL，生成的单二进制文件零外部依赖。

> **路由器存储建议**：为避免频繁写入路由器闪存，建议将 Agent 数据目录指向内存盘：
>
> Master 离线不会影响 Agent 正常工作。关闭的连接日志会先安全写入路由器本地数据库，再尝试上报 Master；上报失败仅记录调试日志，数据不会丢失。批量同步会在检测到 Master 恢复后自动补传所有积压记录，无需手动处理。

### 路由器部署方式

项目提供了两种路由器部署脚本，位于 `backend/scripts/openwrt/`：

**方式 A：便捷启动脚本（临时/调试）**

适合快速验证或不想写 init 脚本的场景，使用 `nohup` 后台运行：

```bash
# 复制到路由器
scp backend/target/x86_64-unknown-linux-musl/release/mihomo-tracker root@192.168.1.1:/usr/bin/
# pscp.exe -scp backend/target/x86_64-unknown-linux-musl/release/mihomo-tracker root@192.168.1.1:/usr/bin/
scp backend/scripts/run_agent_openwrt.sh root@192.168.1.1:/usr/bin/mihomo-tracker-agent.sh
# pscp.exe -scp backend/scripts/run_agent_openwrt.sh root@192.168.1.1:/usr/bin/mihomo-tracker-agent.sh
ssh root@192.168.1.1 chmod +x /usr/bin/mihomo-tracker-agent.sh

# 编辑脚本顶部的 MASTER_URL、MASTER_TOKEN、AGENT_ID 等配置
vi /usr/bin/mihomo-tracker-agent.sh

# 启动
/usr/bin/mihomo-tracker-agent.sh start

# 查看状态 / 停止
/usr/bin/mihomo-tracker-agent.sh status
/usr/bin/mihomo-tracker-agent.sh stop
```

> 注意：SSH 断开后进程仍然存活（`nohup` 保证），但设备重启后需要手动重新启动。

**方式 B：procd init 脚本（推荐生产环境）**

OpenWrt 标准服务管理方式，支持开机自启、崩溃自动重启：

```bash
# 复制二进制和 init 脚本
scp backend/target/x86_64-unknown-linux-musl/release/mihomo-tracker root@192.168.1.1:/usr/bin/
scp backend/scripts/openwrt/init.d/mihomo-tracker root@192.168.1.1:/etc/init.d/mihomo-tracker
ssh root@192.168.1.1 chmod +x /etc/init.d/mihomo-tracker /usr/bin/mihomo-tracker

# 编辑脚本顶部的配置（MASTER_URL、MASTER_TOKEN、AGENT_ID 等）
vi /etc/init.d/mihomo-tracker

# 启用开机自启并启动
/etc/init.d/mihomo-tracker enable
/etc/init.d/mihomo-tracker start

# 查看状态 / 停止 / 重启
/etc/init.d/mihomo-tracker status
/etc/init.d/mihomo-tracker stop
/etc/init.d/mihomo-tracker restart
```

`procd` 参数说明：
- `respawn 3600 5 7`：进程退出后 5 秒内自动重启，1 小时内最多 7 次，超过则放弃。
- `pidfile`：标准 PID 文件路径，便于状态查询。

> 不要直接用 `&` 后台运行 Agent！SSH 断开或终端关闭时进程会被 SIGTERM 终止，且不会自动重启。

## 常用命令

### Backend

```bash
cd backend
cargo build --release
cargo fmt --all
cargo clippy --all
cargo test --all
```

### Frontend

```bash
cd frontend
npm run dev
npm run build
npm run lint --all
npm run test --all
```

### 交叉编译示例（x86_64-unknown-linux-musl）

```bash
rustup target add x86_64-unknown-linux-musl
cd backend
cargo build --release --target x86_64-unknown-linux-musl
# cargo zigbuild --release --target x86_64-unknown-linux-musl    
```

编译完成后，`target/x86_64-unknown-linux-musl/release/mihomo-tracker` 可直接复制到路由器运行。


## API 一览

已实现接口：

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

详细字段和参数见 [api.md](./api.md)。

## 相关文档

- [架构说明](./ARCHITECTURE.md)
- [项目总览](./PROJECT_OVERVIEW.md)
