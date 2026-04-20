"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import {
  LineChart,
  Line,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
} from "recharts";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useApiPolling } from "@/hooks/use-api-polling";
import { ErrorBanner } from "@/components/error-banner";
import { EmptyState } from "@/components/empty-state";
import { Skeleton } from "@/components/ui/skeleton";
import { fetchTimeSeriesStats, type TimeSeriesInterval, type TimeSeriesMetric } from "@/lib/api";
import { formatBytes } from "@/lib/utils";
import type { FilterCriteria } from "@/types/api";
import { format, parseISO } from "date-fns";
import { zhCN } from "date-fns/locale";
import { RefreshCw } from "lucide-react";
import { useMasterTimezone } from "@/app/timezone-context";

const intervals: Array<{ value: TimeSeriesInterval; label: string }> = [
  { value: "minute", label: "分钟" },
  { value: "hour", label: "小时" },
  { value: "day", label: "天" },
  { value: "week", label: "周" },
  { value: "month", label: "月" },
];

const metrics: Array<{ value: TimeSeriesMetric; label: string }> = [
  { value: "connections", label: "连接数" },
  { value: "download", label: "下载" },
  { value: "upload", label: "上传" },
  { value: "total", label: "总流量" },
];

function formatTimePoint(time: string, interval: string, offsetMinutes: number): string {
  try {
    const date = parseISO(time);
    const msOffset = offsetMinutes * 60 * 1000;
    const zonedTimestamp = date.getTime() + msOffset;
    const zonedDate = new Date(zonedTimestamp);
    if (interval === "minute") return format(zonedDate, "MM-dd HH:mm", { locale: zhCN });
    if (interval === "hour") return format(zonedDate, "MM-dd HH:00", { locale: zhCN });
    if (interval === "day") return format(zonedDate, "MM-dd", { locale: zhCN });
    if (interval === "week") return format(zonedDate, "yyyy 第 ww 周", { locale: zhCN });
    if (interval === "month") return format(zonedDate, "yyyy-MM", { locale: zhCN });
    return time;
  } catch (err) {
    console.error("Failed to parse time point:", time, err);
    return time;
  }
}

function getDefaultRangeForInterval(interval: TimeSeriesInterval): { from: Date; to: Date } {
  const now = new Date();
  switch (interval) {
    case "minute":
      return { from: new Date(now.getTime() - 6 * 60 * 60 * 1000), to: now };
    case "hour":
      return { from: new Date(now.getTime() - 24 * 60 * 60 * 1000), to: now };
    case "day":
      return { from: new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000), to: now };
    case "week":
      return { from: new Date(now.getTime() - 12 * 7 * 24 * 60 * 60 * 1000), to: now };
    case "month":
      return { from: new Date(now.getTime() - 12 * 30 * 24 * 60 * 60 * 1000), to: now };
    default:
      return { from: new Date(now.getTime() - 24 * 60 * 60 * 1000), to: now };
  }
}

const LS_INTERVAL_KEY = "dashboard-ts-interval";
const LS_METRIC_KEY = "dashboard-ts-metric";

function usePersistedState<T extends string>(key: string, defaultValue: T): [T, (v: T) => void] {
  const [state, setState] = useState<T>(() => {
    if (typeof window === "undefined") return defaultValue;
    try {
      const stored = window.localStorage.getItem(key);
      return stored ? (stored as T) : defaultValue;
    } catch (err) {
      console.warn("localStorage access failed:", err);
      return defaultValue;
    }
  });

  useEffect(() => {
    try {
      window.localStorage.setItem(key, state);
    } catch (err) {
      console.warn("localStorage setItem failed:", err);
    }
  }, [key, state]);

  return [state, setState];
}

interface TimeSeriesChartProps {
  filters: FilterCriteria;
  /** 外部传入的时序数据，若提供则组件不再自己请求 */
  externalData?: { time: string; value: number }[];
  /** 外部传入的加载状态，配合 externalData 使用 */
  externalLoading?: boolean;
}

export function TimeSeriesChart({ filters, externalData, externalLoading }: TimeSeriesChartProps) {
  const { timezone } = useMasterTimezone();
  const [interval, setInterval] = usePersistedState<TimeSeriesInterval>(LS_INTERVAL_KEY, "hour");
  const [metric, setMetric] = usePersistedState<TimeSeriesMetric>(LS_METRIC_KEY, "total");

  const effectiveFilters = useMemo(() => {
    const hasExplicitRange = filters.from && filters.to;
    if (hasExplicitRange) {
      return filters;
    }
    const { from, to } = getDefaultRangeForInterval(interval);
    return {
      ...filters,
      from: filters.from || from.toISOString(),
      to: filters.to || to.toISOString(),
    };
  }, [filters, interval]);

  const fetcher = useMemo(
    () =>
      (signal?: AbortSignal) =>
        fetchTimeSeriesStats(effectiveFilters, {
          interval,
          metric,
        }, signal),
    [effectiveFilters, interval, metric]
  );

  const { data: selfData, isLoading: selfLoading, error, refetch } = useApiPolling(fetcher, 30000, true);

  const data = externalData ?? selfData;
  const isLoading = externalLoading ?? selfLoading;

  const chartData = useMemo(() => {
    if (!data) return [];
    const offset = timezone?.offset_minutes ?? 0;
    return data.map((item) => ({
      time: formatTimePoint(item.time, interval, offset),
      value: Number(item.value ?? 0),
    }));
  }, [data, interval, timezone]);

  const yFormatter = useCallback(
    (value: number) => {
      if (metric === "connections") return value.toLocaleString("zh-CN");
      return formatBytes(value);
    },
    [metric]
  );

  const validIntervals: TimeSeriesInterval[] = ["minute", "hour", "day", "week", "month"];
  const validMetrics: TimeSeriesMetric[] = ["connections", "download", "upload", "total"];

  const handleIntervalChange = (value: string | null) => {
    const v = value || "hour";
    if (!validIntervals.includes(v as TimeSeriesInterval)) {
      console.warn("Invalid interval:", v);
      return;
    }
    setInterval(v as TimeSeriesInterval);
  };

  const handleMetricChange = (value: string | null) => {
    const v = value || "total";
    if (!validMetrics.includes(v as TimeSeriesMetric)) {
      console.warn("Invalid metric:", v);
      return;
    }
    setMetric(v as TimeSeriesMetric);
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-base font-semibold">时间序列</h3>
        <div className="flex items-center gap-2">
          <Select value={interval} onValueChange={handleIntervalChange} disabled={isLoading}>
            <SelectTrigger className="w-24">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {intervals.map((i) => (
                <SelectItem key={i.value} value={i.value}>
                  {i.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <Select value={metric} onValueChange={handleMetricChange} disabled={isLoading}>
            <SelectTrigger className="w-28">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {metrics.map((m) => (
                <SelectItem key={m.value} value={m.value}>
                  {m.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </div>

      {error ? (
        <ErrorBanner message={error.message} onRetry={refetch} isLoading={isLoading} />
      ) : null}

      {isLoading && !data ? (
        <Skeleton className="h-72" />
      ) : chartData.length === 0 ? (
        <EmptyState height="lg" />
      ) : (
        <div className="relative h-72 w-full">
          <ResponsiveContainer width="100%" height="100%">
            <LineChart data={chartData} margin={{ left: 8, right: 16, top: 8, bottom: 8 }}>
              <CartesianGrid strokeDasharray="3 3" />
              <XAxis
                dataKey="time"
                tick={{ fontSize: 10 }}
                interval="preserveStartEnd"
                minTickGap={16}
              />
              <YAxis
                tick={{ fontSize: 10 }}
                tickFormatter={yFormatter}
                width={metric === "connections" ? 40 : 60}
              />
              <Tooltip
                formatter={(value) => [
                  yFormatter(Number(value ?? 0)),
                  metrics.find((m) => m.value === metric)?.label || metric,
                ]}
                labelStyle={{ fontSize: 12 }}
              />
              <Line
                type="monotone"
                dataKey="value"
                stroke="hsl(var(--chart-1))"
                strokeWidth={2}
                dot={false}
                activeDot={{ r: 4 }}
              />
            </LineChart>
          </ResponsiveContainer>

          {isLoading && (
            <div className="absolute inset-0 flex items-center justify-center rounded-md bg-background/60">
              <RefreshCw className="h-6 w-6 animate-spin text-muted-foreground" />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
