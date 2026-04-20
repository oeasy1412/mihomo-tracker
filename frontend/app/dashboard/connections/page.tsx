"use client";

import { useMemo, useState, useCallback } from "react";
import { useApiPolling } from "@/hooks/use-api-polling";
import { useSort } from "@/hooks/use-sort";
import { usePageSize } from "@/hooks/use-page-size";
import { fetchConnections, type ConnectionSortColumn } from "@/lib/api";
import { formatBytes, formatDateTime, classifyIp, getIpCategoryColor } from "@/lib/utils";
import { defaultFilters } from "@/lib/constants";
import type { FilterCriteria, ConnectionRecord } from "@/types/api";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";
import { FiltersPopover } from "../_components/filters-popover";
import { RefreshButton } from "@/components/refresh-button";
import { ErrorBanner } from "@/components/error-banner";
import { SortableTableHead } from "@/components/sortable-table-head";
import { IconPagination } from "@/components/pagination";
import { PageSizeSelect } from "@/components/page-size-select";
import { PageHeader } from "@/components/page-header";
import { SearchInput } from "@/components/search-input";
import { TableEmptyState, TableLoadingRow } from "@/components/empty-state";

function formatChains(chains: string): string {
  if (!chains) return "-";
  try {
    const parsed = JSON.parse(chains) as string[];
    if (Array.isArray(parsed) && parsed.length > 0) {
      return parsed.join(" -> ");
    }
  } catch (err) {
    console.warn("Failed to parse chains JSON:", chains, err);
  }
  return chains;
}

export default function ConnectionsPage() {
  const [filters, setFilters] = useState<FilterCriteria>(defaultFilters);
  const [search, setSearch] = useState("");
  const sort = useSort<ConnectionSortColumn>("download", "desc");
  const { limit, page, handleLimitChange } = usePageSize(20);

  const isSearching = Boolean(search.trim());

  const fetcher = useCallback(async (signal?: AbortSignal): Promise<{ connections: ConnectionRecord[]; hasMore: boolean }> => {
    const fetchLimit = isSearching ? 5000 : limit + 1;
    const res = await fetchConnections(filters, {
      limit: fetchLimit,
      offset: isSearching ? 0 : page * limit,
      sortBy: sort.sortBy,
      sortOrder: sort.sortOrder,
    }, signal);
    return {
      connections: isSearching ? res : res.slice(0, limit),
      hasMore: res.length === fetchLimit,
    };
  }, [filters, page, limit, sort.sortBy, sort.sortOrder, isSearching]);

  const { data, isLoading, error, refetch } = useApiPolling(fetcher, 10000, true);

  const filteredData = useMemo(() => {
    if (!data) return [];
    if (!search.trim()) return data.connections;
    const q = search.toLowerCase();
    return data.connections.filter(
      (c) =>
        (c.source_ip?.toLowerCase() ?? "").includes(q) ||
        (c.destination_ip?.toLowerCase() ?? "").includes(q) ||
        (c.host?.toLowerCase() ?? "").includes(q) ||
        (c.rule?.toLowerCase() ?? "").includes(q) ||
        (c.process?.toLowerCase() ?? "").includes(q) ||
        (c.agent_id?.toLowerCase() ?? "").includes(q)
    );
  }, [data, search]);

  return (
    <div className="space-y-4">
      <PageHeader title="活跃连接">
        <SearchInput
          value={search}
          onChange={(v) => {
            setSearch(v);
            sort.resetPage();
          }}
          placeholder="搜索 IP / Host / 链路 / 进程 / Agent"
        />
        <FiltersPopover filters={filters} onChange={setFilters} />
        <RefreshButton onClick={refetch} isLoading={isLoading} size="sm" />
      </PageHeader>

      {error && <ErrorBanner message={error.message} onRetry={refetch} isLoading={isLoading} />}

      <div className="overflow-x-auto rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>
                <SortableTableHead
                  label="时间"
                  column="last_updated"
                  sortBy={sort.sortBy}
                  sortOrder={sort.sortOrder}
                  isLoading={isLoading}
                  onSort={sort.handleSort}
                />
              </TableHead>
              <TableHead>源地址</TableHead>
              <TableHead>目标地址</TableHead>
              <TableHead>主机</TableHead>
              <TableHead>链路</TableHead>
              <TableHead>代理链路</TableHead>
              <TableHead className="text-right">
                <SortableTableHead
                  label="下载"
                  column="download"
                  sortBy={sort.sortBy}
                  sortOrder={sort.sortOrder}
                  isLoading={isLoading}
                  onSort={sort.handleSort}
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
                />
              </TableHead>
              <TableHead>Agent</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && !filteredData.length ? (
              <TableLoadingRow colSpan={9} />
            ) : filteredData.length === 0 ? (
              <TableEmptyState colSpan={9} />
            ) : (
              filteredData.map((conn) => {
                const sourceCategory = classifyIp(conn.source_ip);
                const chainsDisplay = formatChains(conn.chains);
                return (
                  <TableRow key={conn.id}>
                    <TableCell className="whitespace-nowrap">
                      {formatDateTime(conn.last_updated)}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs">
                      <div className="flex flex-col gap-0.5">
                        <div className="flex items-center gap-2">
                          <Badge
                            variant="secondary"
                            className={getIpCategoryColor(sourceCategory)}
                          >
                            {sourceCategory}
                          </Badge>
                          <span>{conn.source_ip ? `${conn.source_ip}:${conn.source_port}` : "-"}</span>
                        </div>
                        {conn.process && conn.process !== "-" && (
                          <span className="text-[10px] text-muted-foreground">
                            {conn.process}
                          </span>
                        )}
                      </div>
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs">
                      {conn.destination_ip ? `${conn.destination_ip}:${conn.destination_port}` : "-"}
                    </TableCell>
                    <TableCell className="max-w-[120px] truncate text-xs">
                      {conn.host || "-"}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs">
                      {conn.rule}
                    </TableCell>
                    <TableCell className="max-w-[120px] truncate text-xs" title={chainsDisplay}>
                      {chainsDisplay}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs text-right">
                      {formatBytes(conn.download)}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs text-right">
                      {formatBytes(conn.upload)}
                    </TableCell>
                    <TableCell className="whitespace-nowrap text-xs">
                      {conn.agent_id || "-"}
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div className="flex items-center gap-4">
          <div className="text-sm text-muted-foreground">
            {isSearching ? (
              <>共 {filteredData.length} 条（搜索模式）</>
            ) : (
              <>本页 {filteredData.length} 条</>
            )}
          </div>
          <div className="flex items-center gap-2">
            <span className="text-sm text-muted-foreground">每页</span>
            <PageSizeSelect value={limit} onChange={handleLimitChange} disabled={isSearching} />
          </div>
        </div>
        <IconPagination
          page={page}
          hasPrev={page > 0}
          hasNext={Boolean(data?.hasMore)}
          isLoading={isLoading || isSearching}
          onPageChange={sort.setPage}
        />
      </div>
    </div>
  );
}