"use client";

import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { defaultFilters } from "@/lib/constants";
import { Filter, X } from "lucide-react";
import type { FilterCriteria } from "@/types/api";

interface FiltersPopoverProps {
  filters: FilterCriteria;
  onChange: (filters: FilterCriteria) => void;
}

export function FiltersPopover({ filters, onChange }: FiltersPopoverProps) {
  const [local, setLocal] = useState(filters);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    setLocal(filters);
  }, [filters]);

  const apply = () => {
    onChange(local);
    setOpen(false);
  };

  const reset = () => {
    setLocal(defaultFilters);
    onChange(defaultFilters);
    setOpen(false);
  };

  const hasFilters = Object.values(filters).some((v) => v !== null && v !== "");

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger
        className={cn(
          "inline-flex items-center justify-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          hasFilters
            ? "bg-primary text-primary-foreground hover:bg-primary/90"
            : "border border-input bg-background hover:bg-accent hover:text-accent-foreground"
        )}
      >
        <Filter className="h-4 w-4" />
        过滤器
        {hasFilters && (
          <span className="ml-1 rounded-full bg-primary-foreground px-1.5 py-0.5 text-xs text-primary">
            已启用
          </span>
        )}
      </PopoverTrigger>
      <PopoverContent className="w-80" align="end">
        <div className="grid gap-4">
          <div className="space-y-2">
            <h4 className="font-medium leading-none">过滤条件</h4>
            <p className="text-sm text-muted-foreground">
              设置后将应用到所有视图
            </p>
          </div>
          <div className="grid gap-3">
            <div className="grid grid-cols-2 gap-2">
              <div className="grid gap-1.5">
                <Label htmlFor="from">开始时间</Label>
                <Input
                  id="from"
                  type="datetime-local"
                  value={local.from ?? ""}
                  onChange={(e) =>
                    setLocal({ ...local, from: e.target.value || null })
                  }
                />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="to">结束时间</Label>
                <Input
                  id="to"
                  type="datetime-local"
                  value={local.to ?? ""}
                  onChange={(e) =>
                    setLocal({ ...local, to: e.target.value || null })
                  }
                />
              </div>
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="agentId">Agent ID</Label>
              <Input
                id="agentId"
                placeholder="所有 Agent"
                value={local.agentId ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, agentId: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="network">网络类型</Label>
              <Input
                id="network"
                placeholder="TCP / UDP"
                value={local.network ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, network: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="rule">链路</Label>
              <Input
                id="rule"
                placeholder="DIRECT / 节点选择"
                value={local.rule ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, rule: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="process">进程</Label>
              <Input
                id="process"
                placeholder="chrome"
                value={local.process ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, process: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="source">源地址</Label>
              <Input
                id="source"
                placeholder="源 IP"
                value={local.source ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, source: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="destination">目标地址</Label>
              <Input
                id="destination"
                placeholder="IP 或 Host"
                value={local.destination ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, destination: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="host">主机名</Label>
              <Input
                id="host"
                placeholder="example.com"
                value={local.host ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, host: e.target.value || null })
                }
              />
            </div>
            <div className="grid gap-1.5">
              <Label htmlFor="chains">代理链路</Label>
              <Input
                id="chains"
                placeholder="节点名称"
                value={local.chains ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, chains: e.target.value || null })
                }
              />
            </div>

            <div className="grid gap-1.5">
              <Label htmlFor="destination_port">目标端口</Label>
              <Input
                id="destination_port"
                placeholder="443 / 80"
                value={local.destination_port ?? ""}
                onChange={(e) =>
                  setLocal({ ...local, destination_port: e.target.value || null })
                }
              />
            </div>
          </div>
          <div className="flex items-center justify-between">
            <Button variant="ghost" size="sm" onClick={reset}>
              <X className="mr-1 h-3 w-3" />
              重置
            </Button>
            <Button size="sm" onClick={apply}>
              应用
            </Button>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  );
}
