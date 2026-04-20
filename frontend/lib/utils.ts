import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "-"
  const KB = 1024
  const MB = KB * 1024
  const GB = MB * 1024
  const TB = GB * 1024

  if (bytes >= TB) return `${(bytes / TB).toFixed(2)} TB`
  if (bytes >= GB) return `${(bytes / GB).toFixed(2)} GB`
  if (bytes >= MB) return `${(bytes / MB).toFixed(2)} MB`
  if (bytes >= KB) return `${(bytes / KB).toFixed(2)} KB`
  return `${bytes} B`
}

export function formatDateTime(isoString: string): string {
  if (!isoString) return "-"
  const date = new Date(isoString)
  if (Number.isNaN(date.getTime())) return isoString
  return date.toLocaleString("zh-CN", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  })
}

export type IpCategory = "内网" | "公网" | "回环" | "本地" | "未知";

export function classifyIp(ip: string): IpCategory {
  if (!ip || ip === "-") return "未知";
  if (ip === "::1" || ip.startsWith("127.")) return "回环";
  if (ip.startsWith("192.168.")) return "内网";
  if (ip.startsWith("10.")) return "内网";
  if (ip.startsWith("172.")) {
    const second = parseInt(ip.split(".")[1] || "0", 10);
    if (Number.isNaN(second)) return "未知";
    if (second >= 16 && second <= 31) return "内网";
  }
  if (ip.startsWith("fc") || ip.startsWith("fd") || ip.startsWith("fe80:")) return "内网";
  if (ip.startsWith("::ffff:127.")) return "回环";
  if (ip.startsWith("::ffff:192.168.") || ip.startsWith("::ffff:10.")) return "内网";
  if (ip.startsWith("::ffff:172.")) {
    const parts = ip.split(":");
    if (parts.length === 3) {
      const inner = parts[2];
      const second = parseInt(inner.split(".")[1] || "0", 10);
      if (Number.isNaN(second)) return "未知";
      if (second >= 16 && second <= 31) return "内网";
    }
  }
  if (ip === "0.0.0.0" || ip === "::") return "本地";
  return "公网";
}

export function getIpCategoryColor(category: IpCategory): string {
  switch (category) {
    case "内网":
      return "bg-blue-100 text-blue-700 dark:bg-blue-900 dark:text-blue-200";
    case "公网":
      return "bg-green-100 text-green-700 dark:bg-green-900 dark:text-green-200";
    case "回环":
      return "bg-purple-100 text-purple-700 dark:bg-purple-900 dark:text-purple-200";
    case "本地":
      return "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-200";
    default:
      return "bg-muted text-muted-foreground";
  }
}

export function isIpLike(value: string): boolean {
  if (!value) return false;
  // IPv4
  if (/^(\d{1,3}\.){3}\d{1,3}$/.test(value)) return true;
  // IPv6 (simplified)
  if (/^[0-9a-fA-F:]+$/.test(value) && value.includes(":")) return true;
  return false;
}
