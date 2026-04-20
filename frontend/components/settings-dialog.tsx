"use client";

import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { checkHealth } from "@/lib/api";

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  defaultBaseUrl?: string;
  defaultToken?: string;
  onSave: (baseUrl: string, token: string) => void;
}

export function SettingsDialog({
  open,
  onOpenChange,
  defaultBaseUrl = "",
  defaultToken = "",
  onSave,
}: SettingsDialogProps) {
  const [baseUrl, setBaseUrl] = useState(defaultBaseUrl);
  const [token, setToken] = useState(defaultToken);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<"success" | "error" | null>(null);

  useEffect(() => {
    if (open) {
      setBaseUrl(defaultBaseUrl);
      setToken(defaultToken);
      setTestResult(null);
    }
  }, [open, defaultBaseUrl, defaultToken]);

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    const normalized = baseUrl.trim().replace(/\/$/, "");
    const ok = await checkHealth(normalized);
    setTestResult(ok ? "success" : "error");
    setTesting(false);
  };

  const handleSave = () => {
    const normalized = baseUrl.trim().replace(/\/$/, "");
    onSave(normalized, token.trim());
    onOpenChange(false);
  };

  const isValidBaseUrl = /^https?:\/\/.+/.test(baseUrl.trim());

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>API 设置</DialogTitle>
          <DialogDescription>
            配置后端 Master 节点的地址和认证令牌
          </DialogDescription>
        </DialogHeader>
        <div className="grid gap-4 py-4">
          <div className="grid gap-2">
            <Label htmlFor="baseUrl">API 地址</Label>
            <Input
              id="baseUrl"
              placeholder="http://192.168.1.10:8051"
              value={baseUrl}
              onChange={(e) => setBaseUrl(e.target.value)}
            />
            {baseUrl && !isValidBaseUrl && (
              <p className="text-xs text-destructive">
                地址必须以 http:// 或 https:// 开头
              </p>
            )}
          </div>
          <div className="grid gap-2">
            <Label htmlFor="token">Bearer Token（可选）</Label>
            <Input
              id="token"
              type="password"
              placeholder="your-secret-token"
              value={token}
              onChange={(e) => setToken(e.target.value)}
            />
          </div>
          {testResult === "success" && (
            <p className="text-xs text-green-600">连接成功</p>
          )}
          {testResult === "error" && (
            <p className="text-xs text-destructive">
              连接失败，请检查地址和后端状态
            </p>
          )}
        </div>
        <DialogFooter className="flex gap-2 sm:justify-between">
          <Button
            variant="outline"
            onClick={handleTest}
            disabled={!isValidBaseUrl || testing}
          >
            {testing ? "测试中..." : "测试连接"}
          </Button>
          <Button onClick={handleSave} disabled={!isValidBaseUrl}>
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
