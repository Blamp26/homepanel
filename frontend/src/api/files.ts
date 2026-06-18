import { apiDelete, apiGet, apiPost } from './client';

export type FileKind = 'file' | 'dir' | 'symlink' | 'other';

export type FileEntry = {
  name: string;
  path: string;
  kind: FileKind;
  size: number;
  modified: string | null;
  readonly: boolean;
};

export type FileListResponse = {
  path: string;
  parent_path: string | null;
  allowed_roots: string[];
  entries: FileEntry[];
};

export type FilePreviewResponse = {
  path: string;
  size: number;
  truncated: boolean;
  content: string;
};

export type FileActionResponse = {
  ok: boolean;
  path: string;
};

export type FileDeleteResponse = {
  ok: boolean;
  parent_path: string | null;
};

export async function listFiles(path?: string | null) {
  const search = path ? `?path=${encodeURIComponent(path)}` : '';
  return apiGet<FileListResponse>(`/api/files${search}`);
}

export async function previewFile(path: string) {
  return apiGet<FilePreviewResponse>(`/api/files/preview?path=${encodeURIComponent(path)}`);
}

export async function createFolder(path: string, name: string) {
  return apiPost<FileActionResponse>('/api/files/mkdir', { path, name });
}

export async function renameEntry(path: string, new_name: string) {
  return apiPost<FileActionResponse>('/api/files/rename', { path, new_name });
}

export async function deleteEntry(path: string) {
  return apiDelete<FileDeleteResponse>('/api/files', { path });
}

export async function uploadEntry(path: string, file: File) {
  const formData = new FormData();
  formData.append('file', file);

  const response = await fetch(`/api/files/upload?path=${encodeURIComponent(path)}`, {
    method: 'POST',
    credentials: 'include',
    body: formData,
  });
  if (!response.ok) {
    throw new Error(await response.text());
  }
  return response.json() as Promise<FileActionResponse>;
}

export function downloadEntryUrl(path: string) {
  return `/api/files/download?path=${encodeURIComponent(path)}`;
}
