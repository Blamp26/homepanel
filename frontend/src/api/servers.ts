import { apiDelete, apiGet, apiPatch, apiPost } from './client';

export type ServerStatus = {
  state: 'running' | 'stopped' | 'unknown';
  detail?: string | null;
};

export type ServerRecord = {
  id: string;
  name: string;
  description?: string | null;
  working_dir?: string | null;
  start_script?: string | null;
  stop_script?: string | null;
  restart_script?: string | null;
  log_type?: 'file' | 'journal' | null;
  log_path?: string | null;
  log_unit?: string | null;
  status_type?: 'manual' | 'process' | 'systemd' | 'tcp' | 'http' | null;
  status_value?: string | null;
  status: ServerStatus;
  created_at: string;
  updated_at: string;
};

export type ServerListResponse = {
  items: ServerRecord[];
};

export type ServerActionResult = {
  ok: boolean;
  exit_code?: number | null;
  stdout: string;
  stderr: string;
};

export type ServerLogsResponse = {
  items: string[];
};

export type ServerPayload = {
  name: string;
  description?: string | null;
};

export type ServerUpdatePayload = {
  name?: string;
  description?: string | null;
  working_dir?: string | null;
  start_script?: string | null;
  stop_script?: string | null;
  restart_script?: string | null;
  log_type?: 'file' | 'journal' | null;
  log_path?: string | null;
  log_unit?: string | null;
  status_type?: 'manual' | 'process' | 'systemd' | 'tcp' | 'http' | null;
  status_value?: string | null;
};

export async function listServers() {
  return apiGet<ServerListResponse>('/api/servers');
}

export async function getServer(id: string) {
  return apiGet<ServerRecord>(`/api/servers/${encodeURIComponent(id)}`);
}

export async function createServer(body: ServerPayload) {
  return apiPost<ServerRecord>('/api/servers', body);
}

export async function updateServer(id: string, body: ServerUpdatePayload) {
  return apiPatch<ServerRecord>(`/api/servers/${encodeURIComponent(id)}`, body);
}

export async function deleteServer(id: string) {
  return apiDelete<{ ok: boolean }>(`/api/servers/${encodeURIComponent(id)}`);
}

export async function startServer(id: string) {
  return apiPost<ServerActionResult>(`/api/servers/${encodeURIComponent(id)}/start`);
}

export async function stopServer(id: string) {
  return apiPost<ServerActionResult>(`/api/servers/${encodeURIComponent(id)}/stop`);
}

export async function restartServer(id: string) {
  return apiPost<ServerActionResult>(`/api/servers/${encodeURIComponent(id)}/restart`);
}

export async function getServerLogs(id: string, lines = 200) {
  return apiGet<ServerLogsResponse>(
    `/api/servers/${encodeURIComponent(id)}/logs?lines=${lines}`,
  );
}
