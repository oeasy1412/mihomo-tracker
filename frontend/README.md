# Frontend Dashboard

该目录是 `mihomo-tracker` 的 Next.js 14 前端面板。

## 技术栈

- Next.js 14 (App Router)
- React 18 + TypeScript
- Tailwind CSS + shadcn 风格组件
- Recharts
- Vitest + Testing Library

## 启动

```bash
npm install
npm run dev
```

默认访问：`http://localhost:3000`

## 构建与校验

```bash
npm run build
npm run lint
npm run test
npm run format
npm run format:check
```

## 页面路由

- `/dashboard`：概览（统计卡片、分组统计、时间序列）
- `/dashboard/connections`：连接列表
- `/dashboard/audit`：IP 维度审计
- `/dashboard/logs`：实时日志 + 历史审计日志
- `/dashboard/agents`：Agent 节点状态

## 后端连接配置

前端通过设置弹窗保存以下配置到浏览器本地存储：

- `baseUrl`（例如 `http://127.0.0.1:8051`）
- `token`（若后端启用了 `--api-token`）

API 前缀固定为：`/api/v1`。

## 相关文件

- `lib/api.ts`：HTTP 请求与 WebSocket 封装
- `types/api.ts`：接口类型定义
- `hooks/use-api-polling.ts`：轮询逻辑
- `app/settings-context.tsx`：配置管理
