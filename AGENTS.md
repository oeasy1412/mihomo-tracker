# mihomo-tracker Development Guidelines

**Current Version: v0.1.0**

Generated manually from feature plans. Last updated: 2026-04-16

## Active Technologies
- Rust 1.75+ (后端), TypeScript/React 18 + Next.js 14 (前端) + `tracing`, `tracing-subscriber`, `tracing-appender`, `sqlx`, `warp`, `tokio-tungstenite`, `react-virtuoso` (002-logging-system)
- SQLite (Agent `agent.db`, Master `master.db`) (002-logging-system)

- TypeScript, React 18, Next.js 14 (App Router) + Shadcn UI (Tailwind CSS), Recharts, date-fns (001-frontend-dashboard)
- Rust (Edition 2021) + Tokio + Warp + SQLite (backend master/agent)

完整技术栈选型说明见 [ARCHITECTURE.md](./ARCHITECTURE.md)。

## Project Structure

```text
frontend/          # Next.js 14 web dashboard (new)
backend/           # Rust master/agent server
  src/             # 源代码 (main.rs, api.rs, db.rs, logger.rs 等)
  tests/           # 集成测试
specs/             # Feature specs, plans, and research
```

## Commands

```bash
# Frontend
cd frontend
npm install
npm run dev
npm run build
npm run test
npm run lint
npm run format
npm run format:check

# Backend
cd backend
cargo build --release
cargo fmt --all
cargo clippy --all
cargo test --all
```

## Code Style

- TypeScript / React: Follow standard React hooks rules, prefer explicit return types for API clients, keep components focused and composable.
- Rust: Follow idiomatic Rust conventions, avoid blocking the Tokio runtime, prefer direct SQL aggregates over application-layer reduction.

## Recent Changes

- Agent 清理任务升级: 每小时清理定时任务从单段 DELETE 扩展为四步:
  1. `cleanup_old_records` — 清理 connections 超期数据（`--data-retention-days`，默认 1 天）
  2. `cleanup_synced_connection_logs` — 立即清理已同步的 connection_logs
  3. `cleanup_old_unsynced_connection_logs` — 强制清理超期未同步的 connection_logs（`--log-retention-days`，默认 7 天）
  4. `vacuum_db` — VACUUM 回收磁盘空间
- 添加 tracing 结构化日志，支持文件轮转（app.log.*）和 7 天保留
- 重构审计页面（`/dashboard/audit`）为三层钻取：源 IP → 规则链 → 目标主机/IP，支持面包屑导航和每层时间序列趋势
- 前端代码复用重构，抽离通用组件和工具函数
- 添加 `OpenWrt / ImmortalWrt` x86_64 `musl` 与 Rustls 交叉编译部署指南，推荐闪存友好存储路径（`/tmp/mihomo-tracker/`）
