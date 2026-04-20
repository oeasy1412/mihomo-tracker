"use client";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { LogLevel } from "@/types/api";

const LEVELS: LogLevel[] = ["INFO", "WARN", "ERROR"];

interface LogFiltersProps {
  levels: LogLevel[];
  keyword: string;
  onKeywordChange: (value: string) => void;
  onToggleLevel: (level: LogLevel) => void;
}

export function LogFilters({
  levels,
  keyword,
  onKeywordChange,
  onToggleLevel,
}: LogFiltersProps) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="flex items-center gap-2">
        {LEVELS.map((level) => {
          const active = levels.includes(level);
          return (
            <Button
              key={level}
              variant={active ? "default" : "outline"}
              size="sm"
              onClick={() => onToggleLevel(level)}
            >
              {level}
            </Button>
          );
        })}
      </div>
      <Input
        value={keyword}
        onChange={(e) => onKeywordChange(e.target.value)}
        placeholder="过滤关键词（target / message）"
        className="w-full sm:w-80"
      />
    </div>
  );
}
