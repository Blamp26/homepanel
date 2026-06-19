import { apiGet } from './client';

export type OverviewDisk = {
  mount_point: string;
  total_bytes: number | null;
  used_bytes: number | null;
  available_bytes: number | null;
};

export type OverviewServiceCounts = {
  total: number | null;
  running: number | null;
  failed: number | null;
};

export type OverviewResponse = {
  api_status: string;
  hostname: string | null;
  uptime_seconds: number | null;
  load_average: [number, number, number] | null;
  cpu_usage_percent: number | null;
  memory_total_bytes: number | null;
  memory_available_bytes: number | null;
  memory_used_bytes: number | null;
  disks: OverviewDisk[];
  primary_ips: string[];
  terminal_count: number;
  service_summary: OverviewServiceCounts;
  storage_path: string;
  database_path: string | null;
  version: string;
};

export async function getOverview() {
  return apiGet<OverviewResponse>('/api/overview');
}
