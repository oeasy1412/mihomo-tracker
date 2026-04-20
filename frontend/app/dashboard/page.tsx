"use client";

import { useState } from "react";
import { StatCards } from "./_components/stat-cards";
import { FiltersPopover } from "./_components/filters-popover";
import { GroupedStats } from "./_components/grouped-stats";
import { TimeSeriesChart } from "./_components/time-series-chart";
import { PageHeader } from "@/components/page-header";
import { Button } from "@/components/ui/button";
import { defaultFilters } from "@/lib/constants";
import type { FilterCriteria } from "@/types/api";
import { Zap } from "lucide-react";

export default function DashboardPage() {
  const [filters, setFilters] = useState<FilterCriteria>(defaultFilters);

  return (
    <div className="space-y-6">
      <PageHeader title="总览">
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
      </PageHeader>

      <StatCards filters={filters} />

      <div className="grid gap-6">
        <div className="rounded-md border bg-card p-6">
          <GroupedStats filters={filters} onFilterChange={setFilters} />
        </div>
        <div className="rounded-md border bg-card p-6">
          <TimeSeriesChart filters={filters} />
        </div>
      </div>
    </div>
  );
}
