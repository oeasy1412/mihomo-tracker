"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { cn } from "@/lib/utils";
import { ThemeToggle } from "@/components/theme-toggle";
import { useSettingsDialog } from "@/app/settings-context";
import { Activity, Network, ScrollText, Server, Settings, Shield } from "lucide-react";
import { Button } from "@/components/ui/button";

const navItems = [
  { href: "/dashboard", label: "概览", icon: Activity },
  { href: "/dashboard/connections", label: "连接", icon: Network },
  { href: "/dashboard/audit", label: "审计", icon: Shield },
  { href: "/dashboard/logs", label: "日志", icon: ScrollText },
  { href: "/dashboard/agents", label: "代理节点", icon: Server },
];

export default function DashboardLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const pathname = usePathname();
  const { openSettings } = useSettingsDialog();

  return (
    <div className="min-h-screen flex flex-col bg-background">
      <header className="border-b bg-card px-4 py-3">
        <div className="mx-auto flex max-w-7xl items-center justify-between">
          <div className="flex items-center gap-6">
            <h1 className="text-lg font-semibold">Mihomo 流量监控</h1>
            <nav className="flex items-center gap-1">
              {navItems.map((item) => {
                const isActive =
                  item.href === "/dashboard"
                    ? pathname === item.href
                    : pathname === item.href || pathname.startsWith(`${item.href}/`);
                return (
                  <Link
                    key={item.href}
                    href={item.href}
                    className={cn(
                      "flex items-center gap-1.5 rounded-md px-3 py-1.5 text-sm font-medium transition-colors",
                      isActive
                        ? "bg-primary text-primary-foreground"
                        : "text-muted-foreground hover:bg-muted hover:text-foreground"
                    )}
                  >
                    <item.icon className="h-4 w-4" />
                    {item.label}
                  </Link>
                );
              })}
            </nav>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="icon" onClick={openSettings} title="设置">
              <Settings className="h-4 w-4" />
              <span className="sr-only">设置</span>
            </Button>
            <ThemeToggle />
          </div>
        </div>
      </header>
      <main className="flex-1 px-4 py-6">
        <div className="mx-auto max-w-7xl">{children}</div>
      </main>
    </div>
  );
}
