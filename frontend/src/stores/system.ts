import { writable } from 'svelte/store';

export const systemSummary = writable<Record<string, unknown> | null>(null);
