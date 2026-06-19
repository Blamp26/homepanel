<script lang="ts">
  import { onMount } from 'svelte';
  import { listFiles, type FileEntry, type FileKind } from '../../api/files';

  export let open = false;
  export let title = 'Select a path';
  export let description = 'Browse directories and choose a file or folder.';
  export let initialPath = '';
  export let mode: 'file' | 'directory' = 'file';
  export let confirmLabel = '';
  export let onSelect: (path: string) => void;
  export let onClose: () => void;

  let loading = false;
  let error = '';
  let currentPath = '';
  let parentPath: string | null = null;
  let entries: FileEntry[] = [];
  let addressValue = '';
  let selectedPath: string | null = null;

  function normalizePath(path: string) {
    const trimmed = path.trim();
    if (!trimmed) return '';
    const normalized = trimmed.replace(/\/+/g, '/');
    if (normalized === '/') return '/';
    return normalized.endsWith('/') ? normalized.replace(/\/+$/, '') : normalized;
  }

  function dirname(path: string) {
    const normalized = normalizePath(path);
    if (!normalized || normalized === '/') return '/';
    const parts = normalized.split('/').filter(Boolean);
    parts.pop();
    return parts.length === 0 ? '/' : `/${parts.join('/')}`;
  }

  function slugify(value: string) {
    return value
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');
  }

  function initialDirectory() {
    const normalized = normalizePath(initialPath);
    if (!normalized) return '/';
    if (mode === 'directory') return normalized;
    return normalized.endsWith('/') ? normalized.replace(/\/+$/, '') || '/' : dirname(normalized);
  }

  function compareEntries(a: FileEntry, b: FileEntry) {
    const rank = (kind: FileKind) => {
      switch (kind) {
        case 'dir':
          return 0;
        case 'symlink':
          return 1;
        case 'file':
          return 2;
        default:
          return 3;
      }
    };

    return rank(a.kind) - rank(b.kind) || a.name.localeCompare(b.name);
  }

  function breadcrumbs(path: string) {
    const normalized = normalizePath(path);
    const items = [{ label: '/', path: '/' }];
    if (!normalized || normalized === '/') return items;

    const segments = normalized.split('/').filter(Boolean);
    let current = '';
    for (const segment of segments) {
      current += `/${segment}`;
      items.push({ label: segment, path: current });
    }
    return items;
  }

  async function loadDirectory(path?: string | null) {
    const nextPath = normalizePath(path ?? currentPath ?? initialDirectory()) || '/';
    loading = true;
    error = '';
    try {
      const response = await listFiles(nextPath === '/' ? null : nextPath);
      currentPath = response.path || '/';
      addressValue = response.path || '/';
      parentPath = response.parent_path;
      entries = response.entries.slice().sort(compareEntries);
      if (mode === 'directory') {
        selectedPath = response.path || '/';
      } else if (selectedPath && !entries.some((entry) => entry.path === selectedPath)) {
        selectedPath = null;
      }
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    } finally {
      loading = false;
    }
  }

  async function navigateTo(path: string) {
    const nextPath = normalizePath(path);
    if (!nextPath) return;
    await loadDirectory(nextPath);
  }

  async function goUp() {
    if (!parentPath) return;
    await loadDirectory(parentPath);
  }

  async function submitAddress() {
    const nextPath = normalizePath(addressValue);
    if (!nextPath) {
      error = 'Enter a valid path.';
      addressValue = currentPath || '/';
      return;
    }
    await navigateTo(nextPath);
  }

  function choosePath(path: string) {
    selectedPath = normalizePath(path);
  }

  function closeDialog() {
    if (!loading) onClose();
  }

  function confirmSelection() {
    if (mode === 'directory') {
      onSelect(selectedPath ?? currentPath ?? '/');
      return;
    }

    if (selectedPath) {
      onSelect(selectedPath);
    }
  }

  onMount(() => {
    void loadDirectory(initialDirectory());
  });
</script>

{#if open}
  <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeDialog}></button>
  <section
    class="dialog path-picker-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="path-picker-title"
    aria-describedby="path-picker-description"
    data-testid="path-picker-dialog"
  >
    <div class="dialog-head">
      <div>
        <p class="eyebrow">{mode === 'directory' ? 'Choose folder' : 'Choose file'}</p>
        <h2 id="path-picker-title">{title}</h2>
        <p id="path-picker-description">{description}</p>
      </div>
      <button type="button" class="icon-button" aria-label="Close dialog" disabled={loading} on:click={closeDialog}>x</button>
    </div>

    <div class="picker-toolbar">
      <button type="button" on:click={goUp} disabled={loading || !parentPath}>Up</button>
      <form class="picker-address" on:submit|preventDefault={submitAddress}>
        <input
          data-testid="path-picker-address"
          bind:value={addressValue}
          spellcheck="false"
          autocomplete="off"
          placeholder="/home/superadmin/servers"
        />
        <button type="submit" disabled={loading}>Go</button>
      </form>
    </div>

    <nav class="picker-breadcrumbs" aria-label="Path breadcrumbs" data-testid="path-picker-breadcrumbs">
      {#each breadcrumbs(currentPath) as crumb, index (crumb.path)}
        <button type="button" class="breadcrumb" disabled={loading} on:click={() => navigateTo(crumb.path)}>
          {crumb.label}
        </button>
        {#if index < breadcrumbs(currentPath).length - 1}
          <span aria-hidden="true">/</span>
        {/if}
      {/each}
    </nav>

    {#if error}
      <div class="notice error" role="alert">{error}</div>
    {/if}

    <div class="picker-list" data-testid="path-picker-entries">
      {#if loading && entries.length === 0}
        <div class="picker-empty">Loading…</div>
      {:else if entries.length === 0}
        <div class="picker-empty">This folder is empty.</div>
      {:else}
        {#each entries as entry (entry.path)}
          <div class:selected={selectedPath === entry.path} class="picker-entry">
            <button
              type="button"
              class="picker-entry-main"
              data-testid={`path-picker-entry-${slugify(entry.path)}`}
              on:click={() =>
                entry.kind === 'dir'
                  ? navigateTo(entry.path)
                  : choosePath(entry.path)}
            >
              <span class={`picker-entry-icon ${entry.kind}`}>{entry.kind === 'dir' ? 'dir' : 'file'}</span>
              <span class="picker-entry-copy">
                <strong>{entry.name}</strong>
                <small>{entry.kind === 'dir' ? 'Folder' : 'File'}</small>
              </span>
            </button>

            {#if entry.kind === 'dir'}
              <button
                type="button"
                class="picker-entry-action"
                data-testid={`path-picker-use-${slugify(entry.path)}`}
                disabled={loading}
                on:click={() => choosePath(entry.path)}
              >
                Use
              </button>
            {:else}
              <button
                type="button"
                class="picker-entry-action"
                data-testid={`path-picker-use-${slugify(entry.path)}`}
                disabled={loading}
                on:click={() => choosePath(entry.path)}
              >
                Select
              </button>
            {/if}
          </div>
        {/each}
      {/if}
    </div>

    <div class="picker-selection">
      <span>Selected</span>
      <strong>{selectedPath ?? (mode === 'directory' ? currentPath || '/' : 'None')}</strong>
    </div>

    <div class="dialog-actions">
      <button type="button" on:click={closeDialog} disabled={loading}>Cancel</button>
      <button
        class="primary-button"
        type="button"
        disabled={loading || (mode === 'file' ? !selectedPath : false)}
        data-testid="path-picker-confirm"
        on:click={confirmSelection}
      >
        {confirmLabel || (mode === 'directory' ? 'Use folder' : 'Use file')}
      </button>
    </div>
  </section>
{/if}

<style>
  .dialog-backdrop {
    position: fixed;
    inset: 0;
    z-index: 20;
    border: 0;
    border-radius: 0;
    background: rgba(32, 38, 45, 0.42);
    padding: 0;
  }

  .dialog {
    position: fixed;
    inset: 50% auto auto 50%;
    z-index: 21;
    width: min(100% - 28px, 760px);
    max-height: min(92vh, 840px);
    transform: translate(-50%, -50%);
    display: grid;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface-strong);
    box-shadow: 0 20px 52px rgba(32, 38, 45, 0.2);
  }

  .dialog-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line);
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
  }

  p {
    margin: 3px 0 0;
    color: var(--muted);
    font-size: 0.84rem;
  }

  .icon-button {
    width: 32px;
    height: 32px;
    min-height: 32px;
    display: inline-grid;
    place-items: center;
    padding: 0;
    font-family: var(--mono);
    line-height: 1;
  }

  .picker-toolbar {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 8px;
  }

  .picker-address {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
  }

  .picker-breadcrumbs {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    font-size: 0.8rem;
  }

  .breadcrumb {
    min-height: 28px;
    padding: 0 8px;
  }

  .picker-list {
    min-height: 220px;
    max-height: 44vh;
    overflow: auto;
    display: grid;
    gap: 6px;
    padding: 2px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--surface);
  }

  .picker-empty {
    padding: 18px;
    color: var(--muted);
  }

  .picker-entry {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    align-items: center;
    padding: 8px;
    border-radius: 12px;
  }

  .picker-entry.selected {
    background: #eef3f7;
  }

  .picker-entry-main {
    display: grid;
    grid-template-columns: 28px minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    padding: 0;
    border: 0;
    text-align: left;
    background: transparent;
  }

  .picker-entry-icon {
    display: inline-grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: #e7edf3;
    color: #5f7590;
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .picker-entry-copy {
    display: grid;
    gap: 2px;
  }

  .picker-entry-copy strong {
    overflow-wrap: anywhere;
  }

  .picker-entry-copy small,
  .picker-selection span {
    color: var(--muted);
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .picker-selection {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface);
  }

  .picker-selection strong {
    min-width: 0;
    overflow-wrap: anywhere;
    text-align: right;
  }

  @media (max-width: 720px) {
    .dialog {
      width: min(100% - 20px, 760px);
      max-height: 94vh;
    }

    .picker-toolbar,
    .picker-address,
    .picker-entry,
    .picker-selection,
    .dialog-actions {
      grid-template-columns: 1fr;
      display: grid;
    }

    .picker-selection strong {
      text-align: left;
    }
  }
</style>
