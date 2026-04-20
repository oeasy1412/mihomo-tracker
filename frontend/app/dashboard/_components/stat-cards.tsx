"use client";

import { useMemo } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { useApiPolling } from "@/hooks/use-api-polling";
import { fetchStatsSummary } from "@/lib/api";
import { formatBytes } from "@/lib/utils";
import type { FilterCriteria } from "@/types/api";
import { Activity, ArrowDown, ArrowUp, Database } from "lucide-react";
import { Skeleton } from "@/components/ui/skeleton";
import { ErrorBanner } from "@/components/error-banner";

interface StatCardsProps {
  filters: FilterCriteria;
}

export function StatCards({ filters }: StatCardsProps) {
  const fetcher = useMemo(() => (signal?: AbortSignal) => fetchStatsSummary(filters, signal), [filters]);
  const { data, isLoading, error } = useApiPolling(fetcher, 30000, true);

  if (error) {
    return <ErrorBanner message={error.message} />;
  }

  const items = [
    {
      title: "连接总数",
      value: data?.count ?? 0,
      icon: Activity,
      formatter: (v: number) => v.toLocaleString("zh-CN"),
    },
    {
      title: "总流量",
      value: data?.total ?? 0,
      icon: Database,
      formatter: formatBytes,
    },
    {
      title: "下载",
      value: data?.download ?? 0,
      icon: ArrowDown,
      formatter: formatBytes,
    },
    {
      title: "上传",
      value: data?.upload ?? 0,
      icon: ArrowUp,
      formatter: formatBytes,
    },
  ];

  return (
    <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      {items.map((item) => (
        <Card key={item.title}>
          <CardHeader className="flex flex-row items-center justify-between pb-2">
            <CardTitle className="text-sm font-medium text-muted-foreground">
              {item.title}
            </CardTitle>
            <item.icon className="h-4 w-4 text-muted-foreground" />
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">
              {isLoading && !data ? (
                <Skeleton className="h-8 w-24" />
              ) : (
                item.formatter(item.value)
              )}
            </div>
          </CardContent>
        </Card>
      ))}
    </div>
  );
}
