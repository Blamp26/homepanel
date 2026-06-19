import { apiGet, apiPost } from './client';

export type ServiceSummary = {
  name: string;
  load: string;
  active: string;
  sub: string;
  description: string;
  unit_file_state?: string | null;
};

export type ServiceDetails = {
  name: string;
  load_state: string;
  active_state: string;
  sub_state: string;
  unit_file_state: string;
  description: string;
  fragment_path: string | null;
  main_pid: number;
  memory_current: number | null;
  cpu_usage_nsec: number | null;
};

export type ServiceListResponse = {
  items: ServiceSummary[];
};

export type ServiceLogsResponse = {
  items: string[];
};

export async function listServices() {
  return apiGet<ServiceListResponse>('/api/services');
}

export async function getService(name: string) {
  return apiGet<ServiceDetails>(`/api/services/${encodeURIComponent(name)}`);
}

export async function startService(name: string) {
  return apiPost(`/api/services/${encodeURIComponent(name)}/start`);
}

export async function stopService(name: string) {
  return apiPost(`/api/services/${encodeURIComponent(name)}/stop`);
}

export async function restartService(name: string) {
  return apiPost(`/api/services/${encodeURIComponent(name)}/restart`);
}

export async function getServiceLogs(name: string, lines = 200) {
  return apiGet<ServiceLogsResponse>(
    `/api/services/${encodeURIComponent(name)}/logs?lines=${lines}`,
  );
}
