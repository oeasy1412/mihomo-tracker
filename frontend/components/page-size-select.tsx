"use client";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const pageSizeOptions = [
  { value: "20", label: "20 条/页" },
  { value: "50", label: "50 条/页" },
  { value: "100", label: "100 条/页" },
];

interface PageSizeSelectProps {
  value: number;
  onChange: (value: number) => void;
  disabled?: boolean;
}

export function PageSizeSelect({ value, onChange, disabled }: PageSizeSelectProps) {
  return (
    <Select
      value={String(value)}
      onValueChange={(v) => onChange(Number(v || 20))}
      disabled={disabled}
    >
      <SelectTrigger className="w-28">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        {pageSizeOptions.map((opt) => (
          <SelectItem key={opt.value} value={opt.value}>
            {opt.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
