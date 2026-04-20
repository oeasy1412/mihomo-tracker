"use client";

import { useMemo, useState, useCallback } from "react";
import { useApiPolling } from "@/hooks/use-api-polling";
import { useSort } from "@/hooks/use-sort";
import { usePageSize } from "@/hooks/use-page-size";
import { fetchGroupedStats, fetchTimeSeriesStats, type GroupedStatsSortBy, type ConnectionLogSortColumn } from "@/lib/api";
import { formatBytes, classifyIp, getIpCategoryColor } from "@/lib/utils";
import { TrafficTableCells } from "@/components/traffic-table-cells";
import type { FilterCriteria, GroupByDimension } from "@/types/api";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { defaultFilters } from "@/lib/constants";
import { FiltersPopover } from "../_components/filters-popover";
import { TimeSeriesChart } from "../_components/time-series-chart";
import { RefreshButton } from "@/components/refresh-button";
import { ErrorBanner } from "@/components/error-banner";
import { SortableTableHead } from "@/components/sortable-table-head";
import { TablePaginationFooter } from "@/components/table-pagination-footer";
import { PageHeader } from "@/components/page-header";
import { EmptyState, TableEmptyState, TableLoadingRow } from "@/components/empty-state";
import { Skeleton } from "@/components/ui/skeleton";
import { ChevronLeft, Zap } from "lucide-react";
import {
  BarChart,
  Bar,
  XAxis,
  YAxis,
  Tooltip,
  ResponsiveContainer,
  CartesianGrid,
  Legend,
} from "recharts";

type AuditView = "sources" | "rules" | "targets";

interface TargetRow {
  key: string;
  host: string;
  count: number;
  download: number;
  upload: number;
  total: number;
}

interface AggRow {
  key: string;
  label: string;
  subLabel: string | undefined;
  count: number;
  download: number;
  upload: number;
  total: number;
}

export default function AuditPage() {
  const [filters, setFilters] = useState<FilterCriteria>(defaultFilters);
  const [view, setView] = useState<AuditView>("sources");
  const [selectedSourceIp, setSelectedSourceIp] = useState<string | null>(null);
  const [selectedRule, setSelectedRule] = useState<string | null>(null);

  const aggSort = useSort<GroupedStatsSortBy>("total", "desc");
  const targetSort = useSort<ConnectionLogSortColumn>("download", "desc");
  const { limit, page, setPage, handleLimitChange } = usePageSize(20);
  const aggResetPage = aggSort.resetPage;
  const targetResetPage = targetSort.resetPage;

  const aggFilters = useMemo<FilterCriteria>(() => {
    if (view === "sources") return filters;
    if (view === "rules") return { ...filters, source: selectedSourceIp };
    return { ...filters, source: selectedSourceIp, rule: selectedRule };
  }, [filters, view, selectedSourceIp, selectedRule]);

  const groupBy = useMemo(() => {
    if (view === "sources") return "source" as const;
    return "rule" as const;
  }, [view]);

  const aggFetcher = useMemo(
    () =>
      (signal?: AbortSignal) =>
        fetchGroupedStats(groupBy, aggFilters, {
          sortBy: aggSort.sortBy,
          sortOrder: aggSort.sortOrder,
          limit: view === "targets" ? 999999 : 100,
        }, signal),
    [groupBy, aggFilters, aggSort.sortBy, aggSort.sortOrder, view]
  );

  const { data: aggData, isLoading: aggLoading, error: aggError, refetch: aggRefetch } = useApiPolling(
    aggFetcher,
    30000,
    true
  );

  const targetHostFetcher = useMemo(
    () =>
      (signal?: AbortSignal) =>
        fetchGroupedStats("host" as GroupByDimension, aggFilters, {
          sortBy: targetSort.sortBy as GroupedStatsSortBy,
          sortOrder: targetSort.sortOrder,
          limit: 999999,
        }, signal),
    [aggFilters, targetSort.sortBy, targetSort.sortOrder]
  );

  const { data: targetHostData, isLoading: targetHostLoading, error: targetHostError, refetch: targetHostRefetch } = useApiPolling(
    targetHostFetcher,
    30000,
    true
  );

  const targetRows = useMemo<TargetRow[]>(() => {
    if (view !== "targets" || !targetHostData) return [];
    return targetHostData.map((item) => ({
      key: String(item.host_display ?? item.host ?? item.destination_ip ?? "未知"),
      host: String(item.host_display ?? item.host ?? item.destination_ip ?? "未知"),
      count: Number(item.count ?? 0),
      download: Number(item.download ?? 0),
      upload: Number(item.upload ?? 0),
      total: Number(item.total ?? 0),
    }));
  }, [targetHostData, view]);

  const timeSeriesFetcher = useMemo(
    () =>
      (signal?: AbortSignal) =>
        fetchTimeSeriesStats(aggFilters, { interval: "hour", metric: "total" }, signal),
    [aggFilters]
  );

  const { data: timeSeriesData, isLoading: timeSeriesLoading } = useApiPolling(
    timeSeriesFetcher,
    30000,
    true
  );

  const rows = useMemo<AggRow[]>(() => {
    if (!aggData) return [];
    if (view === "sources") {
      return aggData.map((item) => ({
        key: String(item.source_ip ?? "未知"),
        label: String(item.source_ip ?? "未知"),
        subLabel: undefined,
        count: Number(item.count ?? 0),
        download: Number(item.download ?? 0),
        upload: Number(item.upload ?? 0),
        total: Number(item.total ?? 0),
      }));
    }
    return aggData.map((item) => ({
      key: String(item.rule ?? "未知"),
      label: String(item.rule ?? "未知"),
      subLabel: undefined,
      count: Number(item.count ?? 0),
      download: Number(item.download ?? 0),
      upload: Number(item.upload ?? 0),
      total: Number(item.total ?? 0),
    }));
  }, [aggData, view]);

  const allRows = view === "targets" ? targetRows : rows;
  const totalRows = allRows.length;
  const pagedRows = useMemo<(TargetRow | AggRow)[]>(() => {
    return view === "targets"
      ? targetRows.slice(page * limit, (page + 1) * limit)
      : rows.slice(page * limit, (page + 1) * limit);
  }, [targetRows, rows, page, limit, view]);

  const chartData = useMemo(() => {
    const base = view === "targets" ? targetRows : rows;
    return base.slice(0, 10).map((row) => ({
      key: row.key,
      name: view === "targets"
        ? (row as TargetRow).host.slice(0, 20)
        : (row as AggRow).label.slice(0, 20),
      下载: row.download,
      上传: row.upload,
    }));
  }, [rows, targetRows, view]);

  const navigateToRules = useCallback((sourceIp: string) => {
    setSelectedSourceIp(sourceIp);
    setSelectedRule(null);
    aggResetPage();
    targetResetPage();
    setView("rules");
  }, [aggResetPage, targetResetPage]);

  const navigateToTargets = useCallback((rule: string) => {
    setSelectedRule(rule);
    aggResetPage();
    targetResetPage();
    setView("targets");
  }, [aggResetPage, targetResetPage]);

  const goBackToSources = useCallback(() => {
    setSelectedSourceIp(null);
    setSelectedRule(null);
    aggResetPage();
    targetResetPage();
    setView("sources");
  }, [aggResetPage, targetResetPage]);

  const goBackToRules = useCallback(() => {
    setSelectedRule(null);
    aggResetPage();
    targetResetPage();
    setView("rules");
  }, [aggResetPage, targetResetPage]);

  const handleRowClick = useCallback((key: string) => {
    if (view === "sources") {
      navigateToRules(key);
    } else if (view === "rules") {
      navigateToTargets(key);
    }
  }, [view, navigateToRules, navigateToTargets]);

  const handleSort = useCallback((column: string) => {
    if (view === "targets") {
      targetSort.handleSort(column);
    } else {
      aggSort.handleSort(column);
    }
  }, [view, targetSort, aggSort]);

  const activeSortBy = view === "targets" ? targetSort.sortBy : aggSort.sortBy;
  const activeSortOrder = view === "targets" ? targetSort.sortOrder : aggSort.sortOrder;

  const breadcrumb = useMemo(() => {
    if (view === "sources") {
      return <span className="text-sm text-muted-foreground">按源 IP 分组</span>;
    }
    if (view === "rules") {
      return (
        <div className="flex items-center gap-2 text-sm">
          <Button variant="link" size="sm" className="h-auto p-0 text-muted-foreground" onClick={goBackToSources}>
            源 IP
          </Button>
          <span className="text-muted-foreground">/</span>
          <span className="font-medium">{selectedSourceIp}</span>
          <span className="text-muted-foreground">- 按 Rule 分组</span>
        </div>
      );
    }
    return (
      <div className="flex items-center gap-2 text-sm">
        <Button variant="link" size="sm" className="h-auto p-0 text-muted-foreground" onClick={goBackToSources}>
          源 IP
        </Button>
        <span className="text-muted-foreground">/</span>
        <Button variant="link" size="sm" className="h-auto p-0 text-muted-foreground" onClick={goBackToRules}>
          {selectedSourceIp}
        </Button>
        <span className="text-muted-foreground">/</span>
        <span className="font-medium">{selectedRule}</span>
        <span className="text-muted-foreground">- 目标审计</span>
      </div>
    );
  }, [view, selectedSourceIp, selectedRule, goBackToSources, goBackToRules]);

  const firstColHeader = useMemo(() => {
    if (view === "sources") return "源 IP";
    if (view === "rules") return "Rule 链路";
    return "目标域名 / IP";
  }, [view]);

  const isLoading = view === "targets" ? targetHostLoading : aggLoading;
  const error = view === "targets" ? targetHostError : aggError;
  const refetch = view === "targets" ? targetHostRefetch : aggRefetch;
  const hasPrev = page > 0;
  const hasNext = (page + 1) * limit < totalRows;

  return (
    <div className="space-y-4">
      <PageHeader title="IP 流量审计">
        <div className="space-y-1 hidden sm:block">
          {breadcrumb}
        </div>
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
        <FiltersPopover filters={filters} onChange={setFilters} />
        <RefreshButton onClick={refetch} isLoading={isLoading} />
      </PageHeader>
      <div className="space-y-1 sm:hidden">
        {breadcrumb}
      </div>

      {view !== "sources" && (
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={view === "rules" ? goBackToSources : goBackToRules}>
            <ChevronLeft className="mr-1 h-4 w-4" />
            返回上一级
          </Button>
        </div>
      )}

      {error && (
        <ErrorBanner
          message={error instanceof Error ? error.message : String(error)}
          onRetry={refetch}
          isLoading={isLoading}
        />
      )}

      {isLoading && chartData.length === 0 ? (
        <Skeleton className="h-[28rem]" />
      ) : chartData.length === 0 ? (
        <EmptyState height="lg" />
      ) : (
        <div className="h-[28rem] w-full rounded-md border p-3">
          <ResponsiveContainer width="100%" height="100%">
            <BarChart
              data={chartData}
              layout="vertical"
              margin={{ top: 8, right: 16, left: 16, bottom: 8 }}
            >
              <CartesianGrid strokeDasharray="3 3" horizontal={false} />
              <XAxis type="number" hide />
              <YAxis
                dataKey="name"
                type="category"
                width={120}
                tick={{ fontSize: 11 }}
              />
              <Tooltip
                formatter={(value, name) => [
                  formatBytes(Number(value ?? 0)),
                  String(name),
                ]}
                labelFormatter={() => ""}
              />
              <Legend />
              <Bar
                dataKey="下载"
                stackId="a"
                fill="hsl(var(--chart-1))"
                onClick={(data) => {
                  if (data && typeof data === "object" && "key" in data) {
                    const key = String(data.key);
                    if (view === "targets") return;
                    handleRowClick(key);
                  }
                }}
              />
              <Bar
                dataKey="上传"
                stackId="a"
                fill="hsl(var(--chart-2))"
                onClick={(data) => {
                  if (data && typeof data === "object" && "key" in data) {
                    const key = String(data.key);
                    if (view === "targets") return;
                    handleRowClick(key);
                  }
                }}
              />
            </BarChart>
          </ResponsiveContainer>
        </div>
      )}

      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{firstColHeader}</TableHead>
              <TableHead className="text-right">
                <SortableTableHead
                  label="连接数"
                  column="count"
                  sortBy={activeSortBy as string}
                  sortOrder={activeSortOrder}
                  isLoading={isLoading}
                  onSort={handleSort}
                  className="h-6 px-1"
                />
              </TableHead>
              <TableHead className="text-right">
                <SortableTableHead
                  label="下载"
                  column="download"
                  sortBy={activeSortBy as string}
                  sortOrder={activeSortOrder}
                  isLoading={isLoading}
                  onSort={handleSort}
                  className="h-6 px-1"
                />
              </TableHead>
              <TableHead className="text-right">
                <SortableTableHead
                  label="上传"
                  column="upload"
                  sortBy={activeSortBy as string}
                  sortOrder={activeSortOrder}
                  isLoading={isLoading}
                  onSort={handleSort}
                  className="h-6 px-1"
                />
              </TableHead>
              <TableHead className="text-right">
                <SortableTableHead
                  label="总流量"
                  column="total"
                  sortBy={activeSortBy as string}
                  sortOrder={activeSortOrder}
                  isLoading={isLoading}
                  onSort={handleSort}
                  className="h-6 px-1"
                />
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && pagedRows.length === 0 ? (
              <TableLoadingRow colSpan={5} />
            ) : pagedRows.length === 0 ? (
              <TableEmptyState colSpan={5} />
            ) : view === "targets" ? (
              pagedRows.map((r) => {
                const row = r as TargetRow;
                return (
                  <TableRow key={row.key} className="hover:bg-muted/50">
                    <TableCell className="whitespace-nowrap text-xs">
                      <span>{row.host || "未知"}</span>
                    </TableCell>
                    <TableCell className="text-right text-xs">
                      {row.count.toLocaleString("zh-CN")}
                    </TableCell>
                    <TrafficTableCells
                      download={row.download}
                      upload={row.upload}
                      total={row.total}
                      size="xs"
                    />
                  </TableRow>
                );
              })
            ) : (
              pagedRows.map((r, index) => {
                const row = r as AggRow;
                const isSource = view === "sources";
                const category = isSource ? classifyIp(row.label) : null;
                return (
                  <TableRow
                    key={`${row.key}-${index}`}
                    role="button"
                    tabIndex={0}
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => handleRowClick(row.key)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        handleRowClick(row.key);
                      }
                    }}
                  >
                    <TableCell className="whitespace-nowrap text-xs">
                      <div className="flex flex-col gap-1">
                        <div className="flex items-center gap-2">
                          {isSource && category && (
                            <Badge variant="secondary" className={getIpCategoryColor(category)}>
                              {category}
                            </Badge>
                          )}
                          {!isSource && (
                            <Badge variant="secondary" className="bg-muted text-muted-foreground">
                              {row.label}
                            </Badge>
                          )}
                          {isSource && <span>{row.label}</span>}
                          {!isSource && <span className="font-medium">{row.label}</span>}
                        </div>
                        {row.subLabel && (
                          <span className="text-[10px] text-muted-foreground">
                            {row.subLabel}
                          </span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="text-right text-xs">
                      {row.count.toLocaleString("zh-CN")}
                    </TableCell>
                    <TrafficTableCells
                      download={row.download}
                      upload={row.upload}
                      total={row.total}
                      size="xs"
                    />
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {view !== "sources" && (
        <div className="space-y-3 rounded-md border p-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <h3 className="text-base font-semibold">
                {view === "rules" && selectedSourceIp ? `${selectedSourceIp} 流量趋势` : null}
                {view === "targets" && selectedRule ? `${selectedSourceIp} / ${selectedRule} 流量趋势` : null}
              </h3>
            </div>
          </div>
          <TimeSeriesChart
            filters={aggFilters}
            externalData={timeSeriesData ? timeSeriesData.map(p => ({ time: p.time, value: p.value })) : undefined}
            externalLoading={timeSeriesLoading}
          />
        </div>
      )}

      <TablePaginationFooter
        total={totalRows}
        currentPageSize={pagedRows.length}
        page={page}
        hasPrev={hasPrev}
        hasNext={hasNext}
        isLoading={isLoading}
        onPageChange={setPage}
        onPageSizeChange={handleLimitChange}
      />
    </div>
  );
}