import { apiGet } from './client';

export async function systemSummary() {
  return apiGet('/api/system/summary');
}
