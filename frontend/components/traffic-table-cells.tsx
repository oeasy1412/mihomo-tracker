"use client";

import { TableCell } from "@/components/ui/table";
import { formatBytes } from "@/lib/utils";

interface TrafficTableCellsProps {
  download: number;
  upload: number;
  total: number;
  size?: "xs" | "sm";
  totalClassName?: string;
}

function toSafe(v: unknown): number {
  const n = Number(v);
  return Number.isNaN(n) ? 0 : n;
}

export function TrafficTableCells({
  download,
  upload,
  total,
  size = "sm",
  totalClassName,
}: TrafficTableCellsProps) {
  const textClass = size === "xs" ? "text-xs" : "text-sm";
  return (
    <>
      <TableCell className={`text-right ${textClass} whitespace-nowrap`}>
        {formatBytes(toSafe(download))}
      </TableCell>
      <TableCell className={`text-right ${textClass} whitespace-nowrap`}>
        {formatBytes(toSafe(upload))}
      </TableCell>
      <TableCell
        className={`text-right ${textClass} whitespace-nowrap font-medium ${totalClassName ?? ""}`}
      >
        {formatBytes(toSafe(total))}
      </TableCell>
    </>
  );
}