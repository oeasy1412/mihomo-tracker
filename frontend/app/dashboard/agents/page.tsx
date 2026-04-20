"use client";

import { useMemo, useState } from "react";
import { useApiPolling } from "@/hooks/use-api-polling";
import { fetchAgents } from "@/lib/api";
import { formatBytes, formatDateTime, cn } from "@/lib/utils";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Skeleton } from "@/components/ui/skeleton";
import { AgentDetail } from "./_components/agent-detail";
import { RefreshButton } from "@/components/refresh-button";
import { PageHeader } from "@/components/page-header";
import { SearchInput } from "@/components/search-input";
import { EmptyState } from "@/components/empty-state";
import { ErrorBanner } from "@/components/error-banner";
import { Button } from "@/components/ui/button";
import { Zap } from "lucide-react";
import { useState as useGlobalState } from "react";
import { defaultFilters } from "@/lib/constants";
import type { FilterCriteria } from "@/types/api";

function isAgentActive(lastActive: string): boolean {
  const last = new Date(lastActive).getTime();
  if (Number.isNaN(last)) {
    console.warn("Invalid lastActive timestamp:", lastActive);
    return false;
  }
  return Date.now() - last < 10 * 60 * 1000;
}

export default function AgentsPage() {
  const [search, setSearch] = useState("");
  const [selectedAgent, setSelectedAgent] = useState<string | null>(null);
  const [filters, setFilters] = useGlobalState<FilterCriteria>(defaultFilters);
  const fetcher = useMemo(() => (signal?: AbortSignal) => fetchAgents(filters.exclude_rule ?? undefined, signal), [filters.exclude_rule]);
  const { data, isLoading, error, refetch } = useApiPolling(fetcher, 30000, true);

  const filtered = useMemo(() => {
    if (!data) return [];
    if (!search.trim()) return data;
    return data.filter((a) =>
      a.id.toLowerCase().includes(search.toLowerCase())
    );
  }, [data, search]);

  return (
    <div className="space-y-4">
      <PageHeader title="代理节点">
        <SearchInput
          value={search}
          onChange={setSearch}
          placeholder="搜索 Agent ID"
          className="w-full sm:w-64"
        />
        <Button
          variant={filters.exclude_rule === "DIRECT" ? "default" : "outline"}
          size="sm"
          onClick={() =>
            setFilters((prev) => ({
              ...prev,
              exclude_rule: prev.exclude_rule === "DIRECT" ? null : "DIRECT",
            }))
          }
        >
          <Zap className="mr-1 h-3 w-3" />
          排除 DIRECT
        </Button>
        <RefreshButton onClick={refetch} isLoading={isLoading} />
      </PageHeader>

      {error && (
        <ErrorBanner message={error.message} onRetry={refetch} isLoading={isLoading} />
      )}

      {isLoading && !filtered.length ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <Skeleton key={i} className="h-40" />
          ))}
        </div>
      ) : filtered.length === 0 && !error ? (
        <EmptyState message="暂无代理节点数据" height="lg" />
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {filtered.map((agent) => {
            const active = isAgentActive(agent.last_active);
            return (
              <Card
                key={agent.id}
                role="button"
                tabIndex={isLoading ? -1 : 0}
                aria-disabled={isLoading}
                className={cn("cursor-pointer transition-shadow hover:shadow-md", isLoading && "pointer-events-none opacity-60")}
                onClick={() => !isLoading && setSelectedAgent(agent.id)}
                onKeyDown={(e) => {
                  if (!isLoading && (e.key === "Enter" || e.key === " ")) {
                    e.preventDefault();
                    setSelectedAgent(agent.id);
                  }
                }}
              >
                <CardHeader className="flex flex-row items-center justify-between pb-2">
                  <CardTitle className="text-sm font-medium truncate max-w-[70%]">
                    {agent.id}
                  </CardTitle>
                  <Badge variant={active ? "default" : "secondary"}>
                    {active ? "在线" : "离线"}
                  </Badge>
                </CardHeader>
                <CardContent className="space-y-1 text-sm">
                  <div className="flex justify-between text-muted-foreground">
                    <span>最后活跃</span>
                    <span>{formatDateTime(agent.last_active)}</span>
                  </div>
                  <div className="flex justify-between text-muted-foreground">
                    <span>连接数</span>
                    <span>{agent.connections_count.toLocaleString("zh-CN")}</span>
                  </div>
                  <div className="flex justify-between text-muted-foreground">
                    <span>总流量</span>
                    <span>{formatBytes(agent.total_traffic)}</span>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      <AgentDetail
        agentId={selectedAgent}
        open={Boolean(selectedAgent)}
        onClose={() => setSelectedAgent(null)}
        excludeRule={filters.exclude_rule}
      />
    </div>
  );
}
