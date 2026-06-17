import { apiGet, apiPost } from './client';

export type MeResponse = { username: string };
export type AuthStatusResponse = {
  setup_required: boolean;
  authenticated: boolean;
  username: string | null;
};

export async function status() {
  return apiGet<AuthStatusResponse>('/api/auth/status');
}

export async function me() {
  return apiGet<MeResponse>('/api/auth/me');
}

export async function login(username: string, password: string) {
  return apiPost('/api/auth/login', { username, password });
}

export async function setup(username: string, password: string) {
  return apiPost('/api/auth/setup', { username, password });
}

export async function logout() {
  return apiPost('/api/auth/logout');
}
