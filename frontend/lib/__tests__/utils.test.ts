import { describe, it, expect } from "vitest";
import { formatBytes, formatDateTime, classifyIp, getIpCategoryColor } from "@/lib/utils";

describe("formatBytes", () => {
  it("returns bytes for values < 1024", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
  });

  it("returns KB for values >= 1024 and < 1024^2", () => {
    expect(formatBytes(1024)).toBe("1.00 KB");
    expect(formatBytes(1536)).toBe("1.50 KB");
  });

  it("returns MB for values >= 1024^2 and < 1024^3", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.00 MB");
    expect(formatBytes(2.5 * 1024 * 1024)).toBe("2.50 MB");
  });

  it("returns GB for values >= 1024^3 and < 1024^4", () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.00 GB");
  });

  it("returns TB for values >= 1024^4", () => {
    expect(formatBytes(1024 * 1024 * 1024 * 1024)).toBe("1.00 TB");
  });
});

describe("formatDateTime", () => {
  it("formats ISO string to locale string", () => {
    const result = formatDateTime("2024-01-15T08:30:00.000Z");
    expect(result).toContain("2024");
    expect(result).toContain("15");
  });

  it("returns '-' for empty string", () => {
    expect(formatDateTime("")).toBe("-");
  });

  it("returns original string for invalid date", () => {
    expect(formatDateTime("not-a-date")).toBe("not-a-date");
  });
});

describe("classifyIp", () => {
  it("classifies loopback addresses", () => {
    expect(classifyIp("127.0.0.1")).toBe("回环");
    expect(classifyIp("::1")).toBe("回环");
    expect(classifyIp("::ffff:127.0.0.1")).toBe("回环");
  });

  it("classifies private IPv4 ranges", () => {
    expect(classifyIp("192.168.1.1")).toBe("内网");
    expect(classifyIp("10.0.0.1")).toBe("内网");
    expect(classifyIp("172.16.0.1")).toBe("内网");
    expect(classifyIp("172.31.255.255")).toBe("内网");
  });

  it("classifies public addresses", () => {
    expect(classifyIp("8.8.8.8")).toBe("公网");
    expect(classifyIp("1.1.1.1")).toBe("公网");
  });

  it("classifies local addresses", () => {
    expect(classifyIp("0.0.0.0")).toBe("本地");
    expect(classifyIp("::")).toBe("本地");
  });

  it("returns unknown for empty or invalid input", () => {
    expect(classifyIp("")).toBe("未知");
    expect(classifyIp("-")).toBe("未知");
  });
});

describe("getIpCategoryColor", () => {
  it("returns expected color classes", () => {
    expect(getIpCategoryColor("内网")).toContain("blue");
    expect(getIpCategoryColor("公网")).toContain("green");
    expect(getIpCategoryColor("回环")).toContain("purple");
    expect(getIpCategoryColor("本地")).toContain("gray");
    expect(getIpCategoryColor("未知")).toContain("muted");
  });
});
