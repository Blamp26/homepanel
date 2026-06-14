import { writable } from 'svelte/store';

export const currentUser = writable<string | null>(null);
