import type { FilterCriteria } from "@/types/api";

export const defaultFilters: FilterCriteria = {
  from: null,
  to: null,
  agentId: null,
  network: null,
  rule: null,
  process: null,
  source: null,
  destination: null,
  host: null,
  chains: null,
  destination_port: null,
  exclude_rule: null,
};
