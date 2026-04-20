"use client";

import { useEffect, useState, useRef, useCallback } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { fetchAgentStatus, ApiError } from "@/lib/api";
import { formatBytes } from "@/lib/utils";
import { ErrorBanner } from "@/components/error-banner";
import { Skeleton } from "@/components/ui/skeleton";
import { EmptyState } from "@/components/empty-state";
import type { AgentStatus } from "@/types/api";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  Cell,
} from "recharts";

interface AgentDetailProps {
  agentId: string | null;
  open: boolean;
  onClose: () => void;
  excludeRule?: string | null;
}

export function AgentDetail({ agentId, open, onClose, excludeRule }: AgentDetailProps) {
  const [data, setData] = useState<AgentStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generationRef = useRef<number>(0);
  const controllerRef = useRef<AbortController | null>(null);

  const load = useCallback((id: string, signal: AbortSignal, gen: number) => {
    setLoading(true);
    setError(null);
    fetchAgentStatus(id, excludeRule ?? undefined, signal)
      .then((res) => {
        if (gen === generationRef.current) {
          setData(res);
        }
      })
      .catch((err) => {
        if (err instanceof Error && err.name === "AbortError") return;
        if (gen === generationRef.current) {
          setData(null);
          setError(err instanceof ApiError ? err.message : "加载失败，请稍后重试");
        }
      })
      .finally(() => {
        if (gen === generationRef.current) {
          setLoading(false);
        }
      });
  }, [excludeRule]);

  useEffect(() => {
    if (!agentId || !open) return;
    const gen = ++generationRef.current;
    const controller = new AbortController();
    controllerRef.current = controller;
    load(agentId, controller.signal, gen);
    return () => {
      controller.abort();
      controllerRef.current = null;
    };
  }, [agentId, open, load]);

  const networkData =
    data?.networks.map((n) => ({ name: n.network, value: n.count })) || [];
  const ruleData =
    data?.rules.map((r) => ({ name: r.rule, value: r.count })) || [];

  const handleRetry = () => {
    if (!agentId) return;
    const gen = ++generationRef.current;
    if (controllerRef.current) {
      controllerRef.current.abort();
    }
    const controller = new AbortController();
    controllerRef.current = controller;
    load(agentId, controller.signal, gen);
  };

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Agent 详情: {agentId}</DialogTitle>
        </DialogHeader>
        {loading ? (
          <div className="space-y-4">
            <Skeleton className="h-6 w-1/2" />
            <Skeleton className="h-32" />
            <Skeleton className="h-32" />
          </div>
        ) : error ? (
          <ErrorBanner message={error} onRetry={handleRetry} />
        ) : !data ? (
          <EmptyState message="暂无数据" height="sm" />
        ) : (
          <div className="space-y-6">
            <div className="grid grid-cols-2 gap-4 text-sm">
              <div className="rounded-md border p-3">
                <div className="text-muted-foreground">连接数</div>
                <div className="text-lg font-semibold">
                  {data.connections_count.toLocaleString("zh-CN")}
                </div>
              </div>
              <div className="rounded-md border p-3">
                <div className="text-muted-foreground">总流量</div>
                <div className="text-lg font-semibold">
                  {formatBytes(data.total_traffic)}
                </div>
              </div>
            </div>

            <div>
              <h4 className="mb-2 text-sm font-medium">网络类型分布</h4>
              {networkData.length === 0 ? (
                <EmptyState message="暂无数据" height="sm" />
              ) : (
                <div className="h-40 w-full">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={networkData} margin={{ left: 8, right: 8 }}>
                      <XAxis dataKey="name" tick={{ fontSize: 11 }} />
                      <YAxis tick={{ fontSize: 11 }} />
                      <Tooltip />
                      <Bar dataKey="value" radius={[4, 4, 0, 0]}>
                        {networkData.map((_, i) => (
                          <Cell
                            key={`net-${i}`}
                            fill={`hsl(var(--chart-${(i % 5) + 1}))`}
                          />
                        ))}
                      </Bar>
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              )}
            </div>

            <div>
              <h4 className="mb-2 text-sm font-medium">链路分布</h4>
              {ruleData.length === 0 ? (
                <EmptyState message="暂无数据" height="sm" />
              ) : (
                <div className="h-40 w-full">
                  <ResponsiveContainer width="100%" height="100%">
                    <BarChart data={ruleData} margin={{ left: 8, right: 8 }}>
                      <XAxis dataKey="name" tick={{ fontSize: 11 }} />
                      <YAxis tick={{ fontSize: 11 }} />
                      <Tooltip />
                      <Bar dataKey="value" radius={[4, 4, 0, 0]}>
                        {ruleData.map((_, i) => (
                          <Cell
                            key={`rule-${i}`}
                            fill={`hsl(var(--chart-${(i % 5) + 1}))`}
                          />
                        ))}
                      </Bar>
                    </BarChart>
                  </ResponsiveContainer>
                </div>
              )}
            </div>
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}
