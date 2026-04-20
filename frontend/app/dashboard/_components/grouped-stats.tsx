"use client";

import { useMemo, useState } from "react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
  Cell,
} from "recharts";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { useApiPolling } from "@/hooks/use-api-polling";
import { useSort } from "@/hooks/use-sort";
import { fetchGroupedStats, type GroupedStatsSortBy } from "@/lib/api";
import { formatBytes, cn } from "@/lib/utils";
import { TrafficTableCells } from "@/components/traffic-table-cells";
import type { FilterCriteria, GroupedStatItem, GroupByDimension } from "@/types/api";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { ErrorBanner } from "@/components/error-banner";
import { SortableTableHead } from "@/components/sortable-table-head";
import { EmptyState } from "@/components/empty-state";
import { Skeleton } from "@/components/ui/skeleton";

function getItemLabel(item: GroupedStatItem, dimension: GroupByDimension): string {
  switch (dimension) {
    case "destination":
      return String(item.destination_ip || item.host_display || "未知");
    case "chains":
    case "node":
      return String(item.node ?? "未知");
    case "source":
      return String(item.source_ip ?? "未知");
    case "host":
      return String(item.host ?? "未知");
    case "network":
      return String(item.network ?? "未知");
    case "process":
      return String(item.process ?? "未知");
    case "rule":
      return String(item.rule ?? "未知");
    case "destination_port":
      return String(item.destination_port ?? "未知");
    default:
      return "未知";
  }
}

const dimensionFilterMap: Record<GroupByDimension, keyof FilterCriteria | null> = {
  network: "network",
  rule: "rule",
  process: "process",
  destination: "destination",
  host: "host",
  chains: "chains",
  node: null,
  source: "source",
  destination_port: "destination_port",
};

const dimensions: Array<{ value: GroupByDimension; label: string }> = [
  { value: "network", label: "网络类型" },
  { value: "rule", label: "链路" },
  { value: "source", label: "源 IP" },
  { value: "process", label: "进程" },
  { value: "destination", label: "目标IP" },
  { value: "destination_port", label: "目标端口" },
  { value: "chains", label: "代理链路" },
];

interface GroupedStatsProps {
  filters: FilterCriteria;
  onFilterChange?: (filters: FilterCriteria) => void;
}

export function GroupedStats({ filters, onFilterChange }: GroupedStatsProps) {
  const [dimension, setDimension] = useState<GroupByDimension>("rule");
  const sort = useSort<GroupedStatsSortBy>("total", "desc");

  const fetcher = useMemo(
    () => (signal?: AbortSignal) => fetchGroupedStats(dimension, filters, { limit: 10, sortBy: sort.sortBy, sortOrder: sort.sortOrder }, signal),
    [dimension, filters, sort.sortBy, sort.sortOrder]
  );
  const { data, isLoading, error } = useApiPolling(fetcher, 30000, true);

  const handleApplyFilter = (value: string) => {
    const filterKey = dimensionFilterMap[dimension];
    if (!filterKey || !onFilterChange) return;
    onFilterChange({ ...filters, [filterKey]: value });
  };

  const chartData = useMemo(() => {
    if (!Array.isArray(data)) {
      if (data !== null && data !== undefined) {
        console.error("Expected array for grouped stats, received:", typeof data, data);
      }
      return [];
    }
    return data.map((item) => {
      const label = getItemLabel(item, dimension);
      return {
        name: label.slice(0, 20),
        filterValue: label,
        total: Number(item.total ?? 0),
        count: Number(item.count ?? 0),
      };
    });
  }, [data, dimension]);

  const canFilter = !!dimensionFilterMap[dimension] && !!onFilterChange;
  const isFilterableValue = (value: string) => value !== "未知";

  return (
    <div className="space-y-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <h3 className="text-base font-semibold">分组统计</h3>
        <div className="flex items-center gap-1 overflow-x-auto pb-1">
          {dimensions.map((d) => (
            <Button
              key={d.value}
              variant={dimension === d.value ? "default" : "outline"}
              size="sm"
              className="h-7 px-2 text-xs whitespace-nowrap"
              disabled={isLoading}
              onClick={() => setDimension(d.value)}
            >
              {d.label}
            </Button>
          ))}
        </div>
      </div>

      {error ? (
        <ErrorBanner message={error.message} />
      ) : isLoading && !data ? (
        <Skeleton className="h-48" />
      ) : chartData.length === 0 ? (
        <EmptyState height="lg" />
      ) : (
        <>
          <div className="h-72 w-full">
            <ResponsiveContainer width="100%" height="100%">
              <BarChart data={chartData} layout="vertical" margin={{ left: 16, right: 16 }}>
                <CartesianGrid strokeDasharray="3 3" horizontal={false} />
                <XAxis type="number" hide />
                <YAxis dataKey="name" type="category" width={100} tick={{ fontSize: 11 }} />
                <Tooltip
                  formatter={(value) => formatBytes(Number(value ?? 0))}
                  labelFormatter={() => ""}
                />
                <Bar dataKey="total" radius={[4, 4, 4, 4]}>
                  {chartData.map((entry, index) => {
                    const clickable = canFilter && isFilterableValue(entry.filterValue);
                    return (
                      <Cell
                        key={`cell-${index}`}
                        fill={`hsl(var(--chart-${(index % 5) + 1}))`}
                        className={cn(clickable && "cursor-pointer focus:outline-none focus:ring-2 focus:ring-ring")}
                        role={clickable ? "button" : undefined}
                        tabIndex={clickable ? 0 : undefined}
                        onClick={() => clickable && handleApplyFilter(entry.filterValue)}
                        onKeyDown={(e) => {
                          if (clickable && (e.key === "Enter" || e.key === " ")) {
                            e.preventDefault();
                            handleApplyFilter(entry.filterValue);
                          }
                        }}
                      />
                    );
                  })}
                </Bar>
              </BarChart>
            </ResponsiveContainer>
          </div>

          <div className="max-h-72 overflow-auto rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>分组</TableHead>
                  <TableHead className="text-right">
                    <SortableTableHead
                      label="连接数"
                      column="count"
                      sortBy={sort.sortBy}
                      sortOrder={sort.sortOrder}
                      isLoading={isLoading}
                      onSort={sort.handleSort}
                      className="h-6 px-1"
                    />
                  </TableHead>
                  <TableHead className="text-right">
                    <SortableTableHead
                      label="下载"
                      column="download"
                      sortBy={sort.sortBy}
                      sortOrder={sort.sortOrder}
                      isLoading={isLoading}
                      onSort={sort.handleSort}
                      className="h-6 px-1"
                    />
                  </TableHead>
                  <TableHead className="text-right">
                    <SortableTableHead
                      label="上传"
                      column="upload"
                      sortBy={sort.sortBy}
                      sortOrder={sort.sortOrder}
                      isLoading={isLoading}
                      onSort={sort.handleSort}
                      className="h-6 px-1"
                    />
                  </TableHead>
                  <TableHead className="text-right">
                    <SortableTableHead
                      label="总流量"
                      column="total"
                      sortBy={sort.sortBy}
                      sortOrder={sort.sortOrder}
                      isLoading={isLoading}
                      onSort={sort.handleSort}
                      className="h-6 px-1"
                    />
                  </TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {data?.map((item, index) => {
                  const name = getItemLabel(item, dimension);
                  return (
                    <TableRow key={`${name}-${index}`}>
                      <TableCell className="max-w-[120px] truncate text-xs">
                        {canFilter && isFilterableValue(name) ? (
                          <Badge
                            variant="secondary"
                            className="cursor-pointer"
                            onClick={() => handleApplyFilter(name)}
                          >
                            {name}
                          </Badge>
                        ) : (
                          name
                        )}
                      </TableCell>
                      <TableCell className="text-right text-xs">
                        {Number(item.count ?? 0).toLocaleString("zh-CN")}
                      </TableCell>
                      <TrafficTableCells
                        download={item.download ?? 0}
                        upload={item.upload ?? 0}
                        total={item.total ?? 0}
                        size="xs"
                      />
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </div>
        </>
      )}
    </div>
  );
}
