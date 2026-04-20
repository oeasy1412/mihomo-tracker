"use client";

import { useCallback, useMemo, useState } from "react";
import { useApiConfig } from "@/hooks/use-api-config";
import { useApiPolling } from "@/hooks/use-api-polling";
import { useSort } from "@/hooks/use-sort";
import { usePageSize } from "@/hooks/use-page-size";
import { fetchConnectionLogs, type ConnectionLogSortColumn } from "@/lib/api";
import { formatDateTime } from "@/lib/utils";
import { TrafficTableCells } from "@/components/traffic-table-cells";
import type { ConnectionLog, LogLevel } from "@/types/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { LogFilters } from "./_components/log-filters";
import { LogStream } from "./_components/log-stream";
import { RefreshButton } from "@/components/refresh-button";
import { ErrorBanner } from "@/components/error-banner";
import { SortableTableHead } from "@/components/sortable-table-head";
import { TablePaginationFooter } from "@/components/table-pagination-footer";
import { PageHeader } from "@/components/page-header";
import { SearchInput } from "@/components/search-input";
import { TableEmptyState, TableLoadingRow } from "@/components/empty-state";

export default function LogsPage() {
  const { baseUrl, token, isConfigured } = useApiConfig();
  const [historyKeyword, setHistoryKeyword] = useState("");
  const [levels, setLevels] = useState<LogLevel[]>(["INFO", "WARN", "ERROR"]);
  const [streamKeyword, setStreamKeyword] = useState("");
  const sort = useSort<ConnectionLogSortColumn>("total", "desc");
  const { limit, page, handleLimitChange } = usePageSize(20);

  const handleHistoryKeywordChange = useCallback((value: string) => {
    setHistoryKeyword(value);
    sort.resetPage();
  }, [sort]);

  const historyFetcher = useCallback(
    (signal?: AbortSignal) =>
      fetchConnectionLogs(
        {
          keyword: historyKeyword.trim() || undefined,
        },
        {
          limit,
          offset: page * limit,
          sortBy: sort.sortBy,
          sortOrder: sort.sortOrder,
        },
        signal
      ),
    [limit, page, sort.sortBy, sort.sortOrder, historyKeyword]
  );

  const { data, isLoading, error, refetch } = useApiPolling(historyFetcher, 30000, true);

  const historyRows = useMemo(() => {
    return data?.items ?? [];
  }, [data?.items]);

  const total = data?.total ?? 0;
  const hasPrev = page > 0;
  const hasNext = (page + 1) * limit < total;

  return (
    <div className="space-y-4">
      <PageHeader title="日志中心">
        <RefreshButton onClick={refetch} isLoading={isLoading} size="sm" />
      </PageHeader>

      {!isConfigured ? (
        <div className="rounded-md border border-dashed p-6 text-sm text-muted-foreground">
          请先在右上角设置 API 地址与 Token，再查看实时日志与历史审计。
        </div>
      ) : (
        <Tabs defaultValue="realtime">
          <TabsList>
            <TabsTrigger value="realtime">实时流</TabsTrigger>
            <TabsTrigger value="history">历史审计</TabsTrigger>
          </TabsList>

          <TabsContent value="realtime" className="space-y-3">
            <LogFilters
              levels={levels}
              keyword={streamKeyword}
              onKeywordChange={setStreamKeyword}
              onToggleLevel={(level) => {
                setLevels((prev) =>
                  prev.includes(level) ? prev.filter((v) => v !== level) : [...prev, level]
                );
              }}
            />
            <LogStream
              baseUrl={baseUrl}
              token={token}
              levels={levels}
              keyword={streamKeyword}
            />
          </TabsContent>

          <TabsContent value="history" className="space-y-3">
            <SearchInput
              value={historyKeyword}
              onChange={handleHistoryKeywordChange}
              placeholder="关键字过滤（host / 链路 / process / source / destination / chains）"
              className="w-full sm:w-[420px]"
            />

            {error && (
              <ErrorBanner message={error.message} onRetry={refetch} isLoading={isLoading} />
            )}

            <div className="rounded-md border">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>源 IP</TableHead>
                    <TableHead>主机</TableHead>
                    <TableHead>链路</TableHead>
                    <TableHead>网络</TableHead>
                    <TableHead className="text-right">
                      <SortableTableHead
                        label="下载"
                        column="download"
                        sortBy={sort.sortBy}
                        sortOrder={sort.sortOrder}
                        isLoading={isLoading}
                        onSort={sort.handleSort}
                        align="right"
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
                        align="right"
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
                        align="right"
                      />
                    </TableHead>
                    <TableHead>
                      <SortableTableHead
                        label="结束时间"
                        column="end"
                        sortBy={sort.sortBy}
                        sortOrder={sort.sortOrder}
                        isLoading={isLoading}
                        onSort={sort.handleSort}
                      />
                    </TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {isLoading && !historyRows.length ? (
                    <TableLoadingRow colSpan={8} height="sm" />
                  ) : historyRows.length === 0 ? (
                    <TableEmptyState colSpan={8} height="sm" />
                  ) : (
                    historyRows.map((item: ConnectionLog) => (
                      <TableRow key={`${item.agent_id}-${item.id}-${item.end}`}>
                        <TableCell className="text-xs">{item.source_ip || "-"}</TableCell>
                        <TableCell className="max-w-[200px] truncate text-xs">
                          {item.host || item.destination_ip || "-"}
                        </TableCell>
                        <TableCell className="max-w-[200px] truncate text-xs">{item.rule}</TableCell>
                        <TableCell className="text-xs">{item.network}</TableCell>
                        <TrafficTableCells
                          download={item.download}
                          upload={item.upload}
                          total={(item.download ?? 0) + (item.upload ?? 0)}
                          size="xs"
                        />
                        <TableCell className="text-xs">{formatDateTime(item.end)}</TableCell>
                      </TableRow>
                    ))
                  )}
                </TableBody>
              </Table>
            </div>

            <TablePaginationFooter
              total={total}
              currentPageSize={historyRows.length}
              page={page}
              hasPrev={hasPrev}
              hasNext={hasNext}
              isLoading={isLoading}
              onPageChange={sort.setPage}
              onPageSizeChange={handleLimitChange}
            />
          </TabsContent>
        </Tabs>
      )}
    </div>
  );
}