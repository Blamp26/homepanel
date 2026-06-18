<script lang="ts">
  import { onMount } from 'svelte';
  import {
    createFolder,
    deleteEntry,
    downloadEntryUrl,
    listFiles,
    previewFile,
    renameEntry,
    uploadEntry,
    type FileEntry,
    type FileKind,
    type FileListResponse,
    type FilePreviewResponse,
  } from '../api/files';

  type SidebarLocation = {
    label: string;
    path: string;
    subtitle?: string;
    icon: 'home' | 'games' | 'server' | 'drive';
  };

  type ContextMenu =
    | {
        x: number;
        y: number;
        mode: 'entry';
        path: string;
        name: string;
        kind: FileKind;
      }
    | {
        x: number;
        y: number;
        mode: 'area';
      };

  let loading = false;
  let error = '';
  let actionLoading = false;
  let actionError = '';
  let currentPath = '';
  let addressValue = '';
  let parentPath: string | null = null;
  let allowedRoots: string[] = [];
  let entries: FileEntry[] = [];
  let selectedPath: string | null = null;
  let searchQuery = '';
  let backStack: string[] = [];
  let forwardStack: string[] = [];
  let uploadInput: HTMLInputElement | null = null;
  let contextMenu: ContextMenu | null = null;

  let newFolderOpen = false;
  let newFolderName = '';
  let renameOpen = false;
  let renameName = '';
  let deleteOpen = false;
  let previewOpen = false;
  let previewLoading = false;
  let previewError = '';
  let previewData: FilePreviewResponse | null = null;
  let uploadLoading = false;

  $: selectedEntry = entries.find((entry) => entry.path === selectedPath) ?? null;
  $: currentPathLabel = currentPath || '/';
  $: quickLocations = buildQuickLocations(allowedRoots);
  $: rootEntries = buildRootEntries(allowedRoots, quickLocations);
  $: filteredEntries = entries.filter((entry) =>
    entry.name.toLowerCase().includes(searchQuery.trim().toLowerCase()),
  );
  $: contextMenuItem = entries.find((entry) => entry.path === getContextEntryPath()) ?? null;

  function normalizePath(path: string) {
    const trimmed = path.trim();
    if (!trimmed) return '';
    const normalized = trimmed.replace(/\/+/g, '/');
    if (normalized === '/') return '/';
    return normalized.endsWith('/') ? normalized.replace(/\/+$/, '') : normalized;
  }

  function pathBasename(path: string) {
    const normalized = normalizePath(path);
    if (normalized === '/') return '/';
    const parts = normalized.split('/').filter(Boolean);
    return parts[parts.length - 1] ?? normalized;
  }

  function slugify(value: string) {
    return value.toLowerCase().replace(/[^a-z0-9]+/g, '-');
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

  function toHumanSize(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    const units = ['KiB', 'MiB', 'GiB', 'TiB'];
    let value = bytes / 1024;
    let unit = units[0];
    for (let index = 1; index < units.length && value >= 1024; index += 1) {
      value /= 1024;
      unit = units[index];
    }
    return `${value.toFixed(value >= 10 ? 0 : 1)} ${unit}`;
  }

  function formatKind(kind: FileKind) {
    switch (kind) {
      case 'dir':
        return 'Folder';
      case 'symlink':
        return 'Link';
      case 'file':
        return 'File';
      default:
        return 'Other';
    }
  }

  function isTextLikeEntry(entry: FileEntry) {
    if (entry.kind !== 'file') return false;
    const name = entry.name.toLowerCase();
    const extension = name.includes('.') ? name.split('.').pop() ?? '' : '';
    return [
      'txt',
      'md',
      'rst',
      'json',
      'toml',
      'yaml',
      'yml',
      'ini',
      'conf',
      'cfg',
      'log',
      'csv',
      'xml',
      'html',
      'htm',
      'css',
      'js',
      'mjs',
      'ts',
      'tsx',
      'jsx',
      'rs',
      'py',
      'sh',
      'service',
      'timer',
      'socket',
      'target',
      'mount',
      'path',
      'slice',
    ].includes(extension) || ['makefile', 'dockerfile', 'readme', 'license'].includes(name);
  }

  function buildQuickLocations(roots: string[]): SidebarLocation[] {
    const findRoot = (predicate: (root: string) => boolean) => roots.find(predicate) ?? null;
    const items: SidebarLocation[] = [];

    const homePath = findRoot((root) => root === '/home' || root.startsWith('/home/'));
    if (homePath) {
      items.push({
        label: 'Home',
        path: homePath,
        subtitle: homePath === '/home' ? undefined : homePath,
        icon: 'home',
      });
    }

    const gamesPath =
      findRoot((root) => root === '/mnt/games' || root.startsWith('/mnt/games/')) ??
      findRoot((root) => /games/i.test(root));
    if (gamesPath) {
      items.push({
        label: 'Games',
        path: gamesPath,
        subtitle: gamesPath === '/mnt/games' ? undefined : gamesPath,
        icon: 'games',
      });
    }

    const serverPath = findRoot((root) => root === '/srv' || root.startsWith('/srv/'));
    if (serverPath) {
      items.push({
        label: 'Server data',
        path: serverPath,
        subtitle: serverPath === '/srv' ? undefined : serverPath,
        icon: 'server',
      });
    }

    return items;
  }

  function buildRootEntries(roots: string[], locations: SidebarLocation[]) {
    const quickPaths = new Set(locations.map((location) => normalizePath(location.path)));
    return roots
      .filter((root) => !quickPaths.has(normalizePath(root)))
      .map((root) => ({
        label: pathBasename(root),
        path: root,
        subtitle: root === '/' ? undefined : root,
        icon: 'drive' as const,
      }));
  }

  function getApiErrorMessage(err: unknown) {
    const raw = err instanceof Error ? err.message : String(err);
    try {
      const parsed = JSON.parse(raw) as unknown;
      if (parsed && typeof parsed === 'object') {
        const payload = parsed as Record<string, unknown>;
        const nestedError = payload.error;
        if (nestedError && typeof nestedError === 'object') {
          const nested = nestedError as Record<string, unknown>;
          if (typeof nested.message === 'string') {
            return nested.message;
          }
        }
        if (typeof payload.message === 'string') {
          return payload.message;
        }
      }
    } catch {
      return raw.replace(/^"|"$/g, '').trim();
    }
    return raw.replace(/^"|"$/g, '').trim();
  }

  function getContextEntryPath() {
    return contextMenu?.mode === 'entry' ? contextMenu.path : '';
  }

  function clearTransientErrors() {
    error = '';
    actionError = '';
  }

  function locationIsSelected(path: string) {
    const normalizedPath = normalizePath(path);
    const normalizedCurrent = normalizePath(currentPath);
    return (
      normalizedCurrent === normalizedPath ||
      normalizedCurrent.startsWith(
        normalizedPath.endsWith('/') ? normalizedPath : `${normalizedPath}/`,
      )
    );
  }

  function closeContextMenu() {
    contextMenu = null;
  }

  function closeDialogs() {
    if (actionLoading) return;
    newFolderOpen = false;
    renameOpen = false;
    deleteOpen = false;
    if (!previewLoading) {
      previewOpen = false;
    }
    actionError = '';
    previewError = '';
  }

  async function loadDirectory(path?: string | null, options?: { replaceHistory?: boolean }) {
    const nextPath = normalizePath(path ?? currentPath ?? '');
    loading = true;
    clearTransientErrors();
    try {
      const response: FileListResponse = await listFiles(nextPath || null);
      if (currentPath && response.path !== currentPath) {
        if (!options?.replaceHistory) {
          backStack = [...backStack, currentPath];
          forwardStack = [];
        }
      } else if (options?.replaceHistory) {
        backStack = [];
        forwardStack = [];
      }

      currentPath = response.path;
      addressValue = response.path;
      parentPath = response.parent_path;
      allowedRoots = response.allowed_roots;
      entries = response.entries.slice().sort(compareEntries);
      searchQuery = '';
      if (selectedPath && !entries.some((entry) => entry.path === selectedPath)) {
        selectedPath = null;
      }
      clearTransientErrors();
      closeContextMenu();
    } catch (err) {
      error = getApiErrorMessage(err);
      addressValue = currentPath || addressValue;
    } finally {
      loading = false;
    }
  }

  async function navigateTo(path: string, options?: { replaceHistory?: boolean }) {
    const nextPath = normalizePath(path);
    if (!nextPath) return;
    await loadDirectory(nextPath, options);
  }

  async function goBack() {
    if (backStack.length === 0) return;
    const previous = backStack[backStack.length - 1];
    backStack = backStack.slice(0, -1);
    if (currentPath) {
      forwardStack = [...forwardStack, currentPath];
    }
    await loadDirectory(previous, { replaceHistory: true });
  }

  async function goForward() {
    if (forwardStack.length === 0) return;
    const next = forwardStack[forwardStack.length - 1];
    forwardStack = forwardStack.slice(0, -1);
    if (currentPath) {
      backStack = [...backStack, currentPath];
    }
    await loadDirectory(next, { replaceHistory: true });
  }

  async function goUp() {
    if (!parentPath) return;
    if (currentPath) {
      backStack = [...backStack, currentPath];
      forwardStack = [];
    }
    await loadDirectory(parentPath, { replaceHistory: true });
  }

  async function refreshDirectory() {
    await loadDirectory(currentPath, { replaceHistory: true });
  }

  async function openEntry(entry: FileEntry) {
    selectedPath = entry.path;
    if (entry.kind === 'dir') {
      if (currentPath) {
        backStack = [...backStack, currentPath];
        forwardStack = [];
      }
      await loadDirectory(entry.path, { replaceHistory: true });
      return;
    }

    if (isTextLikeEntry(entry)) {
      await openPreview(entry);
    }
  }

  function selectEntry(entry: FileEntry) {
    selectedPath = entry.path;
    closeContextMenu();
  }

  async function openPreview(entry: FileEntry) {
    if (!isTextLikeEntry(entry)) return;
    previewOpen = true;
    previewLoading = true;
    previewError = '';
    previewData = null;
    closeContextMenu();
    try {
      previewData = await previewFile(entry.path);
    } catch (err) {
      previewError = getApiErrorMessage(err);
    } finally {
      previewLoading = false;
    }
  }

  function openEntryContextMenu(entry: FileEntry, event: MouseEvent) {
    event.preventDefault();
    selectedPath = entry.path;
    contextMenu = {
      x: Math.min(event.clientX, window.innerWidth - 220),
      y: Math.min(event.clientY, window.innerHeight - 220),
      mode: 'entry',
      path: entry.path,
      name: entry.name,
      kind: entry.kind,
    };
  }

  function openAreaContextMenu(event: MouseEvent) {
    if (event.target !== event.currentTarget) return;
    event.preventDefault();
    selectedPath = null;
    contextMenu = {
      x: Math.min(event.clientX, window.innerWidth - 220),
      y: Math.min(event.clientY, window.innerHeight - 180),
      mode: 'area',
    };
  }

  function openNewFolderDialog() {
    actionError = '';
    newFolderName = '';
    newFolderOpen = true;
    closeContextMenu();
  }

  function openRenameDialog(target?: FileEntry) {
    const entry = target ?? selectedEntry ?? contextMenuItem;
    if (!entry) return;
    actionError = '';
    renameName = entry.name;
    renameOpen = true;
    selectedPath = entry.path;
    closeContextMenu();
  }

  function openDeleteDialog(target?: FileEntry) {
    const entry = target ?? selectedEntry ?? contextMenuItem;
    if (!entry) return;
    actionError = '';
    deleteOpen = true;
    selectedPath = entry.path;
    closeContextMenu();
  }

  async function submitNewFolder() {
    if (!newFolderName.trim()) return;
    actionLoading = true;
    actionError = '';
    try {
      await createFolder(currentPath || '/', newFolderName.trim());
      newFolderOpen = false;
      clearTransientErrors();
      await refreshDirectory();
    } catch (err) {
      actionError = getApiErrorMessage(err);
    } finally {
      actionLoading = false;
    }
  }

  async function submitRename() {
    if (!selectedEntry || !renameName.trim()) return;
    actionLoading = true;
    actionError = '';
    try {
      const response = await renameEntry(selectedEntry.path, renameName.trim());
      renameOpen = false;
      selectedPath = response.path;
      clearTransientErrors();
      await loadDirectory(currentPath, { replaceHistory: true });
    } catch (err) {
      actionError = getApiErrorMessage(err);
    } finally {
      actionLoading = false;
    }
  }

  async function submitDelete() {
    if (!selectedEntry) return;
    actionLoading = true;
    actionError = '';
    try {
      const response = await deleteEntry(selectedEntry.path);
      deleteOpen = false;
      selectedPath = null;
      clearTransientErrors();
      if (response.parent_path) {
        await loadDirectory(response.parent_path, { replaceHistory: true });
      } else {
        await refreshDirectory();
      }
    } catch (err) {
      actionError = getApiErrorMessage(err);
    } finally {
      actionLoading = false;
    }
  }

  function triggerUpload() {
    uploadInput?.click();
  }

  async function handleUploadChange(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    uploadLoading = true;
    actionError = '';
    try {
      await uploadEntry(currentPath || '/', file);
      input.value = '';
      clearTransientErrors();
      await refreshDirectory();
    } catch (err) {
      actionError = getApiErrorMessage(err);
    } finally {
      uploadLoading = false;
    }
  }

  async function goToAddress() {
    const nextPath = normalizePath(addressValue);
    if (!nextPath) {
      error = 'Enter a valid path.';
      addressValue = currentPath;
      return;
    }
    await navigateTo(nextPath);
  }

  async function openQuickLocation(path: string) {
    await navigateTo(path);
  }

  function handleBodyClick() {
    closeContextMenu();
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      closeContextMenu();
      closeDialogs();
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeydown);
    void loadDirectory(undefined, { replaceHistory: true });

    return () => {
      window.removeEventListener('keydown', handleKeydown);
    };
  });
</script>

<svelte:body on:click={handleBodyClick} />

<section class="files-page" aria-label="Files">
  <aside class="files-sidebar" aria-label="Quick locations">
    <div class="sidebar-group">
      <div class="sidebar-caption">Quick access</div>
      <div class="sidebar-list" data-testid="files-quick-locations">
        {#each quickLocations as location (location.label)}
          <button
            type="button"
            class:selected={locationIsSelected(location.path)}
            class="sidebar-row"
            data-testid={`quick-location-${slugify(location.label)}`}
            on:click={() => openQuickLocation(location.path)}
          >
            <span class="sidebar-icon" aria-hidden="true">
              {#if location.icon === 'home'}
                <svg viewBox="0 0 24 24"><path d="M4 10.5 12 4l8 6.5v8a1.5 1.5 0 0 1-1.5 1.5h-4.5V14h-4v6H5.5A1.5 1.5 0 0 1 4 18.5z"/></svg>
              {:else if location.icon === 'games'}
                <svg viewBox="0 0 24 24"><path d="M8.5 9 7 7.5 5.5 9 4 7.5 2.5 9 4 10.5 2.5 12 4 13.5 5.5 12 7 13.5 8.5 12 7 10.5z"/><path d="M8.5 6h7c2.4 0 3.8 1 4.3 2.8l1 3.9c.6 2.4-.6 4.3-2.9 4.3-1 0-1.8-.4-2.6-1.3L13 13.4h-2l-2.5 2.3C7.7 16.6 6.9 17 5.9 17c-2.3 0-3.5-1.9-2.9-4.3l1-3.9C4.7 7 6.1 6 8.5 6z"/><circle cx="15.75" cy="10" r="1.2"/><circle cx="18.25" cy="12.5" r="1.2"/></svg>
              {:else if location.icon === 'server'}
                <svg viewBox="0 0 24 24"><path d="M4 5.5A1.5 1.5 0 0 1 5.5 4h13A1.5 1.5 0 0 1 20 5.5v4A1.5 1.5 0 0 1 18.5 11h-13A1.5 1.5 0 0 1 4 9.5z"/><path d="M4 14.5A1.5 1.5 0 0 1 5.5 13h13a1.5 1.5 0 0 1 1.5 1.5v4a1.5 1.5 0 0 1-1.5 1.5h-13A1.5 1.5 0 0 1 4 18.5z"/><circle cx="8" cy="7.5" r="1"/><circle cx="8" cy="16.5" r="1"/></svg>
              {:else}
                <svg viewBox="0 0 24 24"><path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h4l1.5 2H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5z"/></svg>
              {/if}
            </span>
            <span class="sidebar-copy">
              <strong>{location.label}</strong>
              {#if location.subtitle}
                <small>{location.subtitle}</small>
              {/if}
            </span>
          </button>
        {/each}
      </div>
    </div>

    {#if rootEntries.length > 0}
      <div class="sidebar-group">
        <div class="sidebar-caption">Locations</div>
        <div class="sidebar-list" data-testid="files-roots">
          {#each rootEntries as root (root.path)}
            <button
              type="button"
              class:selected={locationIsSelected(root.path)}
              class="sidebar-row"
              data-testid={`root-location-${slugify(root.path)}`}
              on:click={() => openQuickLocation(root.path)}
            >
              <span class="sidebar-icon" aria-hidden="true">
                <svg viewBox="0 0 24 24"><path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h4l1.5 2H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5z"/></svg>
              </span>
              <span class="sidebar-copy">
                <strong>{root.label}</strong>
                {#if root.subtitle}
                  <small>{root.subtitle}</small>
                {/if}
              </span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </aside>

  <div class="files-main">
    <header class="files-chrome">
      <div class="files-topbar" data-testid="files-topbar">
        <div class="nav-buttons">
          <button type="button" class="nav-button" aria-label="Back" title="Back" disabled={backStack.length === 0 || loading} on:click={goBack}>
            <svg viewBox="0 0 24 24"><path d="m14.5 6-6 6 6 6"/><path d="M9 12h9"/></svg>
          </button>
          <button type="button" class="nav-button" aria-label="Forward" title="Forward" disabled={forwardStack.length === 0 || loading} on:click={goForward}>
            <svg viewBox="0 0 24 24"><path d="m9.5 6 6 6-6 6"/><path d="M15 12H6"/></svg>
          </button>
          <button type="button" class="nav-button" aria-label="Up" title="Up" disabled={!parentPath || loading} on:click={goUp}>
            <svg viewBox="0 0 24 24"><path d="m12 6-5 5"/><path d="m12 6 5 5"/><path d="M12 6v12"/></svg>
          </button>
          <button type="button" class="nav-button" aria-label="Refresh" title="Refresh" disabled={loading} on:click={refreshDirectory}>
            <svg viewBox="0 0 24 24"><path d="M20 11a8 8 0 1 0 1.3 4.4"/><path d="M20 4v7h-7"/></svg>
          </button>
        </div>

        <form class="address-shell" on:submit|preventDefault={goToAddress}>
          <span class="field-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24"><path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h4l1.5 2H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5z"/></svg>
          </span>
          <input
            data-testid="files-address"
            bind:value={addressValue}
            spellcheck="false"
            autocomplete="off"
            aria-label="Path address"
            placeholder="/path/to/folder"
          />
        </form>

        <label class="search-shell">
          <span class="field-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="6.5"/><path d="m16 16 4.5 4.5"/></svg>
          </span>
          <input
            data-testid="files-search"
            type="search"
            bind:value={searchQuery}
            aria-label="Search files"
            placeholder="Search this folder"
          />
        </label>
      </div>

      <div class="files-commandbar">
        <button type="button" class="toolbar-button" disabled={loading} on:click={openNewFolderDialog}>
          <svg viewBox="0 0 24 24"><path d="M12 5v14"/><path d="M5 12h14"/></svg>
          <span>New folder</span>
        </button>
        <button type="button" class="toolbar-button" disabled={uploadLoading || loading} on:click={triggerUpload}>
          <svg viewBox="0 0 24 24"><path d="M12 16V5"/><path d="m7.5 9.5 4.5-4.5 4.5 4.5"/><path d="M5 18.5h14"/></svg>
          <span>{uploadLoading ? 'Uploading...' : 'Upload'}</span>
        </button>
        <div class="commandbar-status">
          <span>{filteredEntries.length} item{filteredEntries.length === 1 ? '' : 's'}</span>
          {#if searchQuery.trim()}
            <span>filtered</span>
          {/if}
        </div>
        <input
          bind:this={uploadInput}
          class="sr-only"
          type="file"
          on:change={handleUploadChange}
        />
      </div>
    </header>

    {#if error}
      <div class="notice error files-inline-message" role="alert">{error}</div>
    {/if}

    {#if actionError}
      <div class="notice error files-inline-message" role="alert">{actionError}</div>
    {/if}

    <div class="files-surface">
      {#if loading && entries.length === 0}
        <div class="empty-state">Loading files…</div>
      {:else}
        <div
          class="file-grid"
          role="group"
          data-testid="files-grid"
          on:contextmenu={openAreaContextMenu}
        >
          {#if filteredEntries.length === 0}
            <div class="empty-state" data-testid="files-empty">
              {#if entries.length === 0}
                This folder is empty.
              {:else}
                No items match the current search.
              {/if}
            </div>
          {:else}
            {#each filteredEntries as entry (entry.path)}
              <button
                type="button"
                class:selected={selectedPath === entry.path}
                class="file-tile"
                data-testid={`file-tile-${slugify(entry.name)}`}
                on:click={() => selectEntry(entry)}
                on:dblclick={() => openEntry(entry)}
                on:contextmenu={(event) => openEntryContextMenu(entry, event)}
              >
                <span class={`tile-icon ${entry.kind} ${isTextLikeEntry(entry) ? 'textlike' : ''}`} aria-hidden="true">
                  {#if entry.kind === 'dir'}
                    <svg viewBox="0 0 24 24"><path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h4l1.5 2H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5z"/></svg>
                  {:else if isTextLikeEntry(entry)}
                    <svg viewBox="0 0 24 24"><path d="M7 3.5h7l4 4v13A1.5 1.5 0 0 1 16.5 22h-9A1.5 1.5 0 0 1 6 20.5v-15A2 2 0 0 1 8 3.5z"/><path d="M14 3.5v4h4"/><path d="M9 12h6"/><path d="M9 15h6"/></svg>
                  {:else}
                    <svg viewBox="0 0 24 24"><path d="M7 3.5h7l4 4v13A1.5 1.5 0 0 1 16.5 22h-9A1.5 1.5 0 0 1 6 20.5v-15A2 2 0 0 1 8 3.5z"/><path d="M14 3.5v4h4"/></svg>
                  {/if}
                </span>
                <span class="tile-name" title={entry.name}>{entry.name}</span>
                <span class="tile-meta">
                  {#if entry.kind === 'dir'}
                    Folder
                  {:else}
                    {toHumanSize(entry.size)}
                  {/if}
                </span>
              </button>
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>

  {#if contextMenu}
    <div
      class="context-menu"
      role="menu"
      tabindex="-1"
      aria-label="File actions"
      style={`left:${contextMenu.x}px; top:${contextMenu.y}px;`}
    >
      {#if contextMenu.mode === 'area'}
        <button type="button" on:click={openNewFolderDialog}>New folder</button>
        <button type="button" on:click={triggerUpload}>Upload</button>
      {:else if contextMenuItem}
        <div class="context-menu-head">
          <strong title={contextMenu.name}>{contextMenu.name}</strong>
          <span>{formatKind(contextMenu.kind)}</span>
        </div>

        {#if contextMenu.kind === 'dir'}
          <button type="button" on:click={() => openEntry(contextMenuItem)}>Open</button>
          <button type="button" on:click={() => openRenameDialog(contextMenuItem)}>Rename</button>
          <button type="button" class="danger-item" on:click={() => openDeleteDialog(contextMenuItem)}>
            Delete
          </button>
        {:else}
          {#if isTextLikeEntry(contextMenuItem)}
            <button type="button" on:click={() => openPreview(contextMenuItem)}>Preview</button>
          {/if}
          <a
            href={downloadEntryUrl(contextMenuItem.path)}
            download={contextMenuItem.name}
            on:click={closeContextMenu}
          >
            Download
          </a>
          <button type="button" on:click={() => openRenameDialog(contextMenuItem)}>Rename</button>
          <button type="button" class="danger-item" on:click={() => openDeleteDialog(contextMenuItem)}>
            Delete
          </button>
        {/if}
      {/if}
    </div>
  {/if}
</section>

{#if newFolderOpen}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close dialog"
    on:click={closeDialogs}
  ></button>
  <form
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="new-folder-title"
    on:submit|preventDefault={submitNewFolder}
  >
    <div class="dialog-head">
      <div>
        <h3 id="new-folder-title">New folder</h3>
        <p>Create a folder inside {currentPathLabel}</p>
      </div>
      <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeDialogs}>x</button>
    </div>

    <label for="new-folder-name">Folder name</label>
    <input
      id="new-folder-name"
      bind:value={newFolderName}
      placeholder="New folder"
      autocomplete="off"
      required
    />

    <div class="dialog-actions">
      <button type="button" on:click={closeDialogs} disabled={actionLoading}>Cancel</button>
      <button class="primary-button" type="submit" disabled={actionLoading}>Create folder</button>
    </div>
  </form>
{/if}

{#if renameOpen && selectedEntry}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close dialog"
    on:click={closeDialogs}
  ></button>
  <form
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="rename-title"
    on:submit|preventDefault={submitRename}
  >
    <div class="dialog-head">
      <div>
        <h3 id="rename-title">Rename item</h3>
        <p>{selectedEntry.path}</p>
      </div>
      <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeDialogs}>x</button>
    </div>

    <label for="rename-name">New name</label>
    <input id="rename-name" bind:value={renameName} autocomplete="off" required />

    <div class="dialog-actions">
      <button type="button" on:click={closeDialogs} disabled={actionLoading}>Cancel</button>
      <button class="primary-button" type="submit" disabled={actionLoading}>Rename</button>
    </div>
  </form>
{/if}

{#if deleteOpen && selectedEntry}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close dialog"
    on:click={closeDialogs}
  ></button>
  <form
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="delete-title"
    on:submit|preventDefault={submitDelete}
  >
    <div class="dialog-head">
      <div>
        <h3 id="delete-title">Delete item</h3>
        <p>This removes empty folders and files only.</p>
      </div>
      <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeDialogs}>x</button>
    </div>

    <div class="confirm-copy">
      Delete <strong>{selectedEntry.name}</strong>?
    </div>

    <div class="dialog-actions">
      <button type="button" on:click={closeDialogs} disabled={actionLoading}>Cancel</button>
      <button class="primary-button danger-button" type="submit" disabled={actionLoading}>Delete</button>
    </div>
  </form>
{/if}

{#if previewOpen}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close preview"
    on:click={() => !previewLoading && (previewOpen = false)}
  ></button>
  <section
    class="dialog preview-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="preview-title"
  >
    <div class="dialog-head">
      <div>
        <h3 id="preview-title">Text preview</h3>
        <p>{previewData?.path ?? 'Loading preview...'}</p>
      </div>
      <button
        type="button"
        class="icon-button"
        aria-label="Close preview"
        disabled={previewLoading}
        on:click={() => (previewOpen = false)}
      >
        x
      </button>
    </div>

    {#if previewLoading}
      <div class="empty-state">Loading preview…</div>
    {:else if previewError}
      <div class="notice error" role="alert">{previewError}</div>
    {:else if previewData}
      <div class="preview-meta">
        <span>{previewData.size} bytes</span>
        {#if previewData.truncated}
          <span>Truncated to 256 KiB</span>
        {/if}
        <a href={downloadEntryUrl(previewData.path)} download={pathBasename(previewData.path)}>Download</a>
      </div>
      <pre class="preview-content">{previewData.content}</pre>
    {/if}
  </section>
{/if}

<style>
  .files-page {
    --explorer-bg: #f3f5f7;
    --explorer-surface: #fafbfc;
    --explorer-line: #d7dee6;
    --explorer-line-strong: #b9c4cf;
    min-height: 0;
    height: 100%;
    display: grid;
    grid-template-columns: 224px minmax(0, 1fr);
    background: var(--explorer-bg);
    overflow: hidden;
  }

  .files-sidebar {
    min-height: 0;
    overflow: auto;
    padding: 10px 8px 12px;
    border-right: 1px solid var(--explorer-line);
    background: #f8f9fb;
    display: grid;
    align-content: start;
    gap: 14px;
  }

  .sidebar-group {
    display: grid;
    gap: 6px;
  }

  .sidebar-caption {
    padding: 0 8px;
    color: #718296;
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .sidebar-list {
    display: grid;
    gap: 2px;
  }

  .sidebar-row {
    min-height: 34px;
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr);
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 10px;
    border: 0;
    border-radius: 10px;
    background: transparent;
    color: var(--text);
    text-align: left;
  }

  .sidebar-row:hover {
    background: #eef2f5;
  }

  .sidebar-row.selected {
    background: #dfeaf6;
  }

  .sidebar-icon {
    width: 18px;
    height: 18px;
    color: #5f7590;
  }

  .sidebar-icon svg,
  .field-icon svg,
  .nav-button svg,
  .toolbar-button svg,
  .tile-icon svg {
    width: 100%;
    height: 100%;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .sidebar-copy {
    min-width: 0;
    display: grid;
    gap: 1px;
  }

  .sidebar-copy strong {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 0.84rem;
    font-weight: 600;
  }

  .sidebar-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #7d8ea3;
    font-size: 0.68rem;
  }

  .files-main {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    gap: 8px;
    padding: 8px 10px 10px 0;
  }

  .files-chrome {
    display: grid;
    gap: 6px;
  }

  .files-topbar,
  .files-commandbar {
    min-height: 40px;
    display: grid;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid var(--explorer-line);
    border-radius: 8px;
    background: var(--explorer-surface);
  }

  .files-topbar {
    grid-template-columns: auto minmax(0, 1fr) minmax(180px, 260px);
  }

  .files-commandbar {
    grid-template-columns: auto auto 1fr;
  }

  .nav-buttons {
    display: flex;
    gap: 6px;
  }

  .nav-button,
  .toolbar-button,
  .icon-button {
    border: 1px solid #cfd7e0;
    background: #ffffff;
    color: #425364;
  }

  .nav-button {
    width: 30px;
    height: 30px;
    display: inline-grid;
    place-items: center;
    padding: 0;
    border-radius: 9px;
  }

  .nav-button svg {
    width: 15px;
    height: 15px;
  }

  .nav-button:disabled {
    opacity: 0.45;
  }

  .address-shell,
  .search-shell {
    min-width: 0;
    min-height: 30px;
    display: grid;
    grid-template-columns: 16px minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    padding: 0 10px;
    border: 1px solid #cfd7e0;
    border-radius: 8px;
    background: #fff;
  }

  .field-icon {
    width: 16px;
    height: 16px;
    color: #708397;
  }

  .address-shell input,
  .search-shell input {
    min-width: 0;
    height: 28px;
    border: 0;
    padding: 0;
    background: transparent;
    box-shadow: none;
    font-size: 0.84rem;
  }

  .address-shell input:focus,
  .search-shell input:focus {
    outline: none;
  }

  .toolbar-button {
    min-height: 30px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 0 10px;
    border-radius: 9px;
    font-size: 0.79rem;
    font-weight: 600;
  }

  .toolbar-button svg {
    width: 14px;
    height: 14px;
  }

  .commandbar-status {
    justify-self: end;
    display: flex;
    gap: 10px;
    color: #718296;
    font-size: 0.74rem;
  }

  .files-inline-message {
    margin: 0;
  }

  .files-surface {
    min-height: 0;
    border: 1px solid var(--explorer-line);
    border-radius: 8px;
    background: #fff;
    overflow: auto;
  }

  .file-grid {
    min-height: 100%;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(98px, 112px));
    align-content: start;
    gap: 14px 12px;
    padding: 16px 18px 20px;
  }

  .file-tile {
    min-height: 110px;
    display: grid;
    grid-template-rows: 52px auto auto;
    justify-items: center;
    align-content: start;
    gap: 8px;
    padding: 10px 8px;
    border: 1px solid transparent;
    border-radius: 14px;
    background: transparent;
    text-align: center;
  }

  .file-tile:hover {
    background: #f3f6f9;
  }

  .file-tile.selected {
    border-color: #b7cadb;
    background: #e7f0f8;
  }

  .tile-icon {
    width: 40px;
    height: 40px;
    display: inline-grid;
    place-items: center;
    color: #7f95ac;
  }

  .tile-icon.dir {
    color: #d7a03a;
  }

  .tile-icon.file {
    color: #6d8bae;
  }

  .tile-icon.textlike {
    color: #5f86c1;
  }

  .tile-name {
    display: -webkit-box;
    width: 100%;
    overflow: hidden;
    color: #243244;
    font-size: 0.78rem;
    line-height: 1.18;
    text-wrap: balance;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
  }

  .tile-meta {
    color: #7a8b9f;
    font-size: 0.68rem;
    line-height: 1.1;
  }

  .empty-state {
    display: grid;
    place-items: center;
    min-height: 220px;
    padding: 28px;
    color: #728395;
    font-size: 0.88rem;
    text-align: center;
  }

  .context-menu {
    position: fixed;
    z-index: 30;
    min-width: 180px;
    padding: 6px;
    border: 1px solid #cfd7e0;
    border-radius: 8px;
    background: #fff;
  }

  .context-menu-head {
    display: grid;
    gap: 2px;
    padding: 4px 6px 8px;
    border-bottom: 1px solid rgba(113, 133, 156, 0.14);
    margin-bottom: 6px;
  }

  .context-menu-head strong,
  .context-menu-head span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .context-menu-head span {
    color: #728395;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .context-menu button,
  .context-menu a {
    width: 100%;
    display: block;
    padding: 8px 10px;
    border-radius: 8px;
    border: 0;
    background: transparent;
    color: #223244;
    text-align: left;
    text-decoration: none;
    font-size: 0.83rem;
  }

  .context-menu button:hover,
  .context-menu a:hover {
    background: #eef2f5;
  }

  .context-menu .danger-item {
    color: var(--danger);
  }

  .dialog-backdrop {
    position: fixed;
    inset: 0;
    z-index: 40;
    border: 0;
    border-radius: 0;
    background: rgba(30, 39, 52, 0.34);
    padding: 0;
  }

  .dialog {
    position: fixed;
    inset: 50% auto auto 50%;
    z-index: 41;
    width: min(100% - 28px, 520px);
    transform: translate(-50%, -50%);
    display: grid;
    gap: 12px;
    padding: 16px;
    border: 1px solid #cfd7e0;
    border-radius: 10px;
    background: #fff;
  }

  .preview-dialog {
    width: min(100% - 28px, 780px);
  }

  .dialog-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding-bottom: 6px;
    border-bottom: 1px solid #e1e7ee;
  }

  .dialog-head h3 {
    margin: 0;
    font-size: 1rem;
  }

  .dialog-head p {
    margin: 4px 0 0;
    color: #708397;
    font-size: 0.84rem;
    word-break: break-word;
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
    border-radius: 10px;
  }

  .dialog label {
    font-size: 0.82rem;
    font-weight: 600;
  }

  .dialog input {
    width: 100%;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .confirm-copy {
    padding: 12px;
    border: 1px solid #c08383;
    border-radius: var(--radius);
    background: var(--danger-soft);
    color: var(--danger);
  }

  .preview-meta {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    color: #708397;
  }

  .preview-meta span,
  .preview-meta a {
    font-size: 0.72rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .preview-meta a {
    color: var(--accent);
    text-decoration: none;
  }

  .preview-content {
    min-height: 280px;
    max-height: min(70vh, 720px);
    margin: 0;
    padding: 12px;
    overflow: auto;
    border: 1px solid #d7dee6;
    border-radius: 8px;
    background: var(--terminal);
    color: #d8dee5;
    font-family: var(--mono);
    font-size: 0.78rem;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
    border: 0;
  }

  @media (max-width: 980px) {
    .files-page {
      grid-template-columns: 196px minmax(0, 1fr);
    }

  .files-topbar {
    grid-template-columns: auto minmax(0, 1fr);
  }

    .search-shell {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 760px) {
    .files-page {
      grid-template-columns: 1fr;
    }

    .files-sidebar {
      border-right: 0;
      border-bottom: 1px solid var(--explorer-line);
    }

    .files-topbar,
    .files-commandbar {
      grid-template-columns: 1fr;
    }

    .nav-buttons {
      order: 1;
    }

    .address-shell {
      order: 2;
    }

    .search-shell {
      order: 3;
      grid-column: auto;
    }

    .commandbar-status {
      justify-self: start;
    }

    .file-grid {
      grid-template-columns: repeat(auto-fill, minmax(92px, 1fr));
      padding: 14px;
    }
  }
</style>
