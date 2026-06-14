import { apiGet, apiPost } from './client';

export type TerminalSummary = {
  id: string;
  name: string;
  kind: string;
  status: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  exit_code?: number | null;
  last_attached_at?: string | null;
};

export async function listTerminals() {
  return apiGet<TerminalSummary[]>('/api/terminals');
}

export async function createTerminal(payload: Record<string, unknown>) {
  return apiPost<TerminalSummary>('/api/terminals', payload);
}

export async function killTerminal(id: string) {
  return apiPost(`/api/terminals/${id}/kill`);
}

export async function clearTerminalScrollback(id: string) {
  return apiPost(`/api/terminals/${id}/clear-scrollback`);
}
