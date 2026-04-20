"use client";

import { useEffect, useMemo, useState } from "react";
import { Virtuoso } from "react-virtuoso";
import { Badge } from "@/components/ui/badge";
import { logStreamSocket } from "@/lib/api";
import { formatBytes, formatDateTime } from "@/lib/utils";
import type { LogLevel, LogStreamEvent } from "@/types/api";

interface LogStreamProps {
  baseUrl: string;
  token: string;
  levels: LogLevel[];
  keyword: string;
}

function levelVariant(level: LogLevel): "default" | "secondary" | "destructive" {
  if (level === "ERROR") return "destructive";
  if (level === "WARN") return "secondary";
  return "default";
}

export function LogStream({ baseUrl, token, levels, keyword }: LogStreamProps) {
  const [events, setEvents] = useState<LogStreamEvent[]>([]);

  useEffect(() => {
    if (!baseUrl) return;
    const close = logStreamSocket(baseUrl, token, (event) => {
      setEvents((prev) => {
        const next = [event, ...prev];
        return next.length > 10_000 ? next.slice(0, 10_000) : next;
      });
    });
    return close;
  }, [baseUrl, token]);

  const filtered = useMemo(() => {
    const q = keyword.trim().toLowerCase();
    return events.filter((event) => {
      if (event.type === "system") {
        const level = event.level ?? "INFO";
        if (!levels.includes(level)) return false;
      }
      if (!q) return true;
      return JSON.stringify(event).toLowerCase().includes(q);
    });
  }, [events, keyword, levels]);

  return (
    <div className="h-[560px] rounded-md border">
      <Virtuoso
        data={filtered}
        itemContent={(_index, event) => {
          if (event.type === "system") {
            const level = event.level ?? "INFO";
            return (
              <div className="border-b px-3 py-2 text-xs">
                <div className="mb-1 flex items-center gap-2">
                  <Badge variant={levelVariant(level)}>{level}</Badge>
                  <span className="text-muted-foreground">{formatDateTime(event.timestamp)}</span>
                  <span className="truncate text-muted-foreground">{event.target ?? "-"}</span>
                </div>
                <p className="break-words leading-relaxed">{event.message ?? "-"}</p>
              </div>
            );
          }

          const conn = event.connection;
          return (
            <div className="border-b px-3 py-2 text-xs">
              <div className="mb-1 flex items-center gap-2">
                <Badge variant="outline">CONNECTION_CLOSED</Badge>
                <span className="text-muted-foreground">{formatDateTime(event.timestamp)}</span>
              </div>
              <p className="break-words leading-relaxed">
                [{conn?.source_ip || "-"}] {conn?.host || conn?.destination_ip || "-"} |{" "}
                {conn?.rule || "-"} | {conn?.network || "-"} | ↑{formatBytes(conn?.upload ?? 0)} / ↓
                {formatBytes(conn?.download ?? 0)}
              </p>
            </div>
          );
        }}
      />
    </div>
  );
}
