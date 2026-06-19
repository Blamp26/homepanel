<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import PathPickerDialog from '../components/files/PathPickerDialog.svelte';
  import {
    createServer,
    deleteServer,
    getServer,
    getServerLogs,
    listServers,
    restartServer,
    startServer,
    stopServer,
    updateServer,
    type ServerActionResult,
    type ServerRecord,
    type ServerUpdatePayload,
  } from '../api/servers';

  const logLineOptions = [100, 200, 500, 1000] as const;
  const statusOptions = [
    { value: 'manual', label: 'Manual / unknown' },
    { value: 'systemd', label: 'systemd unit' },
    { value: 'process', label: 'Process name' },
  ] as const;

  type MetadataFormState = { name: string; description: string };
  type ActionsFormState = {
    working_dir: string;
    start_script: string;
    stop_script: string;
    restart_script: string;
  };
  type LogsFormState = {
    log_mode: 'none' | 'file' | 'journal';
    log_path: string;
    log_unit: string;
  };
  type StatusFormState = {
    status_type: 'manual' | 'systemd' | 'process';
    status_value: string;
  };
  type PathPickerTarget =
    | { field: 'working_dir'; mode: 'directory' }
    | { field: 'start_script'; mode: 'file' }
    | { field: 'stop_script'; mode: 'file' }
    | { field: 'restart_script'; mode: 'file' }
    | { field: 'log_path'; mode: 'file' };

  let loading = false;
  let error = '';
  let servers: ServerRecord[] = [];
  let selectedServerId: string | null = null;
  let selectedServer: ServerRecord | null = null;
  let detailLoading = false;
  let detailError = '';
  let listSearch = '';

  let metadataOpen = false;
  let metadataMode: 'create' | 'edit' = 'create';
  let metadataLoading = false;
  let metadataError = '';
  let metadataServerId: string | null = null;
  let metadataState = createEmptyMetadataForm();

  let actionsOpen = false;
  let actionsLoading = false;
  let actionsError = '';
  let actionsServerId: string | null = null;
  let actionsState = createEmptyActionsForm();

  let logsConfigOpen = false;
  let logsConfigLoading = false;
  let logsConfigError = '';
  let logsConfigServerId: string | null = null;
  let logsConfigState = createEmptyLogsForm();

  let statusConfigOpen = false;
  let statusConfigLoading = false;
  let statusConfigError = '';
  let statusConfigServerId: string | null = null;
  let statusConfigState = createEmptyStatusForm();

  let pathPicker: PathPickerTarget | null = null;

  let deleteOpen = false;
  let deleteLoading = false;
  let deleteError = '';

  let actionLoading: 'start' | 'stop' | 'restart' | null = null;
  let actionError = '';
  let lastActionResult: (ServerActionResult & { action: string }) | null = null;

  let logs: string[] = [];
  let logsLoading = false;
  let logsError = '';
  let logsLines: (typeof logLineOptions)[number] = 200;
  let logsSearch = '';
  let logsAutoRefresh = false;
  let logsAutoRefreshTimer: ReturnType<typeof setInterval> | null = null;
  let logsCopyFeedback = '';

  function createEmptyMetadataForm(): MetadataFormState {
    return { name: '', description: '' };
  }

  function createEmptyActionsForm(): ActionsFormState {
    return { working_dir: '', start_script: '', stop_script: '', restart_script: '' };
  }

  function createEmptyLogsForm(): LogsFormState {
    return { log_mode: 'none', log_path: '', log_unit: '' };
  }

  function createEmptyStatusForm(): StatusFormState {
    return { status_type: 'manual', status_value: '' };
  }

  function slugify(value: string) {
    return value
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '');
  }

  function parseApiError(err: unknown) {
    const raw = err instanceof Error ? err.message : String(err);
    try {
      const parsed = JSON.parse(raw) as {
        message?: string;
        error?: { message?: string };
      };
      return parsed.error?.message ?? parsed.message ?? raw;
    } catch {
      return raw.replace(/^"|"$/g, '').trim();
    }
  }

  function metadataPayload(state: MetadataFormState) {
    return { name: state.name, description: state.description };
  }

  function actionsPayload(state: ActionsFormState): ServerUpdatePayload {
    return {
      working_dir: state.working_dir,
      start_script: state.start_script,
      stop_script: state.stop_script,
      restart_script: state.restart_script,
    };
  }

  function logsPayload(state: LogsFormState): ServerUpdatePayload {
    return {
      log_type: state.log_mode === 'none' ? null : state.log_mode,
      log_path: state.log_mode === 'file' ? state.log_path : null,
      log_unit: state.log_mode === 'journal' ? state.log_unit : null,
    };
  }

  function statusPayload(state: StatusFormState): ServerUpdatePayload {
    return {
      status_type: state.status_type,
      status_value: state.status_type === 'manual' ? null : state.status_value,
    };
  }

  function metadataFormFromServer(server: ServerRecord): MetadataFormState {
    return { name: server.name, description: server.description ?? '' };
  }

  function actionsFormFromServer(server: ServerRecord): ActionsFormState {
    return {
      working_dir: server.working_dir ?? '',
      start_script: server.start_script ?? '',
      stop_script: server.stop_script ?? '',
      restart_script: server.restart_script ?? '',
    };
  }

  function logsFormFromServer(server: ServerRecord): LogsFormState {
    return {
      log_mode: server.log_type ?? 'none',
      log_path: server.log_path ?? '',
      log_unit: server.log_unit ?? '',
    };
  }

  function statusFormFromServer(server: ServerRecord): StatusFormState {
    return {
      status_type:
        server.status_type === 'process' || server.status_type === 'systemd'
          ? server.status_type
          : 'manual',
      status_value: server.status_value ?? '',
    };
  }

  function isActionConfigured(server: ServerRecord | null, action: 'start' | 'stop' | 'restart') {
    if (!server) return false;
    if (action === 'start') return Boolean(server.start_script);
    if (action === 'stop') return Boolean(server.stop_script);
    return Boolean(server.restart_script);
  }

  function actionHelpText(action: 'start' | 'stop' | 'restart') {
    if (!selectedServer || isActionConfigured(selectedServer, action)) return '';
    return `Configure a ${action} script to enable this action.`;
  }

  function hasAnyActions(server: ServerRecord | null) {
    return Boolean(server?.start_script || server?.stop_script || server?.restart_script);
  }

  function hasStatusConfigured(server: ServerRecord | null) {
    return Boolean(server?.status_type && server.status_type !== 'manual');
  }

  function statusTone(state: ServerRecord['status']['state']) {
    switch (state) {
      case 'running':
        return 'good';
      case 'stopped':
        return 'muted';
      default:
        return 'neutral';
    }
  }

  function statusLabel(state: ServerRecord['status']['state']) {
    switch (state) {
      case 'running':
        return 'RUNNING';
      case 'stopped':
        return 'STOPPED';
      default:
        return 'UNKNOWN';
    }
  }

  function actionLabel(action: 'start' | 'stop' | 'restart') {
    if (actionLoading !== action) return action[0].toUpperCase() + action.slice(1);
    if (action === 'start') return 'Starting...';
    if (action === 'stop') return 'Stopping...';
    return 'Restarting...';
  }

  function clearSelectedLogs() {
    logs = [];
    logsError = '';
    logsCopyFeedback = '';
  }

  async function loadLogs(id: string, lines: (typeof logLineOptions)[number] = logsLines) {
    const server =
      selectedServer?.id === id ? selectedServer : servers.find((item) => item.id === id) ?? null;
    if (!server?.log_type) {
      clearSelectedLogs();
      return;
    }

    logsLoading = true;
    logsError = '';
    logsCopyFeedback = '';
    try {
      const response = await getServerLogs(id, lines);
      logs = response.items;
    } catch (err) {
      logsError = parseApiError(err);
      logs = [];
    } finally {
      logsLoading = false;
    }
  }

  async function loadSelectedServer(id: string) {
    detailLoading = true;
    detailError = '';
    try {
      const server = await getServer(id);
      selectedServer = server;
      selectedServerId = server.id;
      metadataState = metadataFormFromServer(server);
      actionsState = actionsFormFromServer(server);
      logsConfigState = logsFormFromServer(server);
      statusConfigState = statusFormFromServer(server);
      await loadLogs(server.id);
    } catch (err) {
      detailError = parseApiError(err);
      selectedServer = null;
      clearSelectedLogs();
    } finally {
      detailLoading = false;
    }
  }

  async function loadServers(preferredId?: string | null) {
    if (loading) return;
    loading = true;
    error = '';
    try {
      const response = await listServers();
      servers = response.items;
      const nextSelectedId =
        preferredId && response.items.some((server) => server.id === preferredId)
          ? preferredId
          : selectedServerId && response.items.some((server) => server.id === selectedServerId)
            ? selectedServerId
            : response.items[0]?.id ?? null;
      selectedServerId = nextSelectedId;
      if (nextSelectedId) {
        await loadSelectedServer(nextSelectedId);
      } else {
        selectedServer = null;
        clearSelectedLogs();
        lastActionResult = null;
      }
    } catch (err) {
      error = parseApiError(err);
      servers = [];
      selectedServerId = null;
      selectedServer = null;
      clearSelectedLogs();
    } finally {
      loading = false;
    }
  }

  async function selectServer(id: string) {
    if (selectedServerId === id && selectedServer) return;
    logsSearch = '';
    lastActionResult = null;
    await loadSelectedServer(id);
  }

  function openCreateForm() {
    metadataMode = 'create';
    metadataOpen = true;
    metadataLoading = false;
    metadataError = '';
    metadataServerId = null;
    metadataState = createEmptyMetadataForm();
  }

  function openEditForm() {
    if (!selectedServer) return;
    metadataMode = 'edit';
    metadataOpen = true;
    metadataLoading = false;
    metadataError = '';
    metadataServerId = selectedServer.id;
    metadataState = metadataFormFromServer(selectedServer);
  }

  function openActionsForm() {
    if (!selectedServer) return;
    actionsOpen = true;
    actionsLoading = false;
    actionsError = '';
    actionsServerId = selectedServer.id;
    actionsState = actionsFormFromServer(selectedServer);
  }

  function openLogsConfigForm() {
    if (!selectedServer) return;
    logsConfigOpen = true;
    logsConfigLoading = false;
    logsConfigError = '';
    logsConfigServerId = selectedServer.id;
    logsConfigState = logsFormFromServer(selectedServer);
  }

  function openStatusConfigForm() {
    if (!selectedServer) return;
    statusConfigOpen = true;
    statusConfigLoading = false;
    statusConfigError = '';
    statusConfigServerId = selectedServer.id;
    statusConfigState = statusFormFromServer(selectedServer);
  }

  function closeMetadataForm() {
    if (!metadataLoading) {
      metadataOpen = false;
      metadataError = '';
    }
  }

  function closeActionsForm() {
    if (!actionsLoading) {
      actionsOpen = false;
      actionsError = '';
    }
  }

  function closeLogsConfigForm() {
    if (!logsConfigLoading) {
      logsConfigOpen = false;
      logsConfigError = '';
    }
  }

  function closeStatusConfigForm() {
    if (!statusConfigLoading) {
      statusConfigOpen = false;
      statusConfigError = '';
    }
  }

  async function submitMetadataForm() {
    metadataLoading = true;
    metadataError = '';
    try {
      const payload = metadataPayload(metadataState);
      const server =
        metadataMode === 'create'
          ? await createServer(payload)
          : await updateServer(metadataServerId ?? '', payload);
      metadataOpen = false;
      await loadServers(server.id);
    } catch (err) {
      metadataError = parseApiError(err);
    } finally {
      metadataLoading = false;
    }
  }

  async function submitActionsForm() {
    if (!actionsServerId) return;
    actionsLoading = true;
    actionsError = '';
    try {
      await updateServer(actionsServerId, actionsPayload(actionsState));
      actionsOpen = false;
      await loadServers(actionsServerId);
    } catch (err) {
      actionsError = parseApiError(err);
    } finally {
      actionsLoading = false;
    }
  }

  async function submitLogsConfigForm() {
    if (!logsConfigServerId) return;
    logsConfigLoading = true;
    logsConfigError = '';
    try {
      await updateServer(logsConfigServerId, logsPayload(logsConfigState));
      logsConfigOpen = false;
      await loadServers(logsConfigServerId);
    } catch (err) {
      logsConfigError = parseApiError(err);
    } finally {
      logsConfigLoading = false;
    }
  }

  async function submitStatusConfigForm() {
    if (!statusConfigServerId) return;
    statusConfigLoading = true;
    statusConfigError = '';
    try {
      await updateServer(statusConfigServerId, statusPayload(statusConfigState));
      statusConfigOpen = false;
      await loadServers(statusConfigServerId);
    } catch (err) {
      statusConfigError = parseApiError(err);
    } finally {
      statusConfigLoading = false;
    }
  }

  function openDeleteDialog() {
    if (!selectedServer) return;
    deleteOpen = true;
    deleteLoading = false;
    deleteError = '';
  }

  function closeDeleteDialog() {
    if (!deleteLoading) {
      deleteOpen = false;
      deleteError = '';
    }
  }

  async function confirmDelete() {
    if (!selectedServer) return;
    deleteLoading = true;
    deleteError = '';
    try {
      await deleteServer(selectedServer.id);
      deleteOpen = false;
      await loadServers(null);
    } catch (err) {
      deleteError = parseApiError(err);
    } finally {
      deleteLoading = false;
    }
  }

  async function runAction(action: 'start' | 'stop' | 'restart', id = selectedServerId) {
    if (!id) return;
    if (selectedServer && selectedServer.id === id && !isActionConfigured(selectedServer, action)) {
      actionError = `Configure a ${action} script before running this action.`;
      return;
    }
    actionLoading = action;
    actionError = '';
    try {
      const result =
        action === 'start'
          ? await startServer(id)
          : action === 'stop'
            ? await stopServer(id)
            : await restartServer(id);
      lastActionResult = { ...result, action };
      await loadServers(id);
    } catch (err) {
      actionError = parseApiError(err);
    } finally {
      actionLoading = null;
    }
  }

  async function updateLogLines(lines: (typeof logLineOptions)[number]) {
    logsLines = lines;
    if (selectedServerId && selectedServer?.log_type) {
      await loadLogs(selectedServerId, lines);
    }
  }

  async function handleLogLineChange(event: Event) {
    await updateLogLines(
      Number((event.currentTarget as HTMLSelectElement).value) as
        (typeof logLineOptions)[number],
    );
  }

  async function copyVisibleLogs() {
    try {
      await navigator.clipboard.writeText(filteredLogs.join('\n'));
      logsCopyFeedback = 'Copied';
    } catch (err) {
      logsCopyFeedback = parseApiError(err);
    }
  }

  function openPathPicker(field: PathPickerTarget['field'], mode: PathPickerTarget['mode']) {
    if (field === 'working_dir' && mode === 'directory') {
      pathPicker = { field, mode };
    } else if (field !== 'working_dir' && mode === 'file') {
      pathPicker = { field, mode };
    }
  }

  function applyPickedPath(path: string) {
    const value = path.trim();
    if (!pathPicker) return;
    if (pathPicker.field === 'working_dir') {
      actionsState = { ...actionsState, working_dir: value };
    } else if (pathPicker.field === 'start_script') {
      actionsState = { ...actionsState, start_script: value };
    } else if (pathPicker.field === 'stop_script') {
      actionsState = { ...actionsState, stop_script: value };
    } else if (pathPicker.field === 'restart_script') {
      actionsState = { ...actionsState, restart_script: value };
    } else if (pathPicker.field === 'log_path') {
      logsConfigState = { ...logsConfigState, log_path: value };
    }
    pathPicker = null;
  }

  function closePathPicker() {
    pathPicker = null;
  }

  $: filteredServers = servers.filter((server) =>
    `${server.name} ${server.description ?? ''}`
      .toLowerCase()
      .includes(listSearch.trim().toLowerCase()),
  );
  $: filteredLogs = logsSearch.trim()
    ? logs.filter((line) => line.toLowerCase().includes(logsSearch.trim().toLowerCase()))
    : logs;
  $: {
    if (logsAutoRefreshTimer) {
      clearInterval(logsAutoRefreshTimer);
      logsAutoRefreshTimer = null;
    }
    if (logsAutoRefresh && selectedServerId && selectedServer?.log_type) {
      const serverId = selectedServerId;
      logsAutoRefreshTimer = setInterval(() => {
        void loadLogs(serverId);
      }, 3_000);
    }
  }

  onMount(() => {
    void loadServers();
  });

  onDestroy(() => {
    if (logsAutoRefreshTimer) {
      clearInterval(logsAutoRefreshTimer);
    }
  });
</script>

<section class="servers-page" aria-label="Servers" data-testid="servers-page">
  <div class="servers-list-panel">
    <div class="panel-head">
      <div>
        <h2>Servers</h2>
        <p>User-defined cards for apps, game servers, helpers, and services.</p>
      </div>
      <button type="button" class="primary-button compact" data-testid="servers-add" on:click={openCreateForm}>
        Add server
      </button>
    </div>

    <div class="servers-toolbar">
      <label class="servers-search-label" for="servers-search">
        <span class="sr-only">Search servers</span>
        <input
          id="servers-search"
          data-testid="servers-search"
          type="search"
          placeholder="Search server cards..."
          bind:value={listSearch}
        />
      </label>
      <button type="button" on:click={() => loadServers(selectedServerId)} disabled={loading}>
        Refresh
      </button>
    </div>

    {#if error}
      <div class="notice error" role="alert">{error}</div>
    {/if}

    {#if !loading && servers.length === 0}
      <div class="servers-empty" data-testid="servers-empty">
        <p class="eyebrow">No servers yet</p>
        <h3>No servers yet. Add a server card and point it at your start/stop scripts.</h3>
        <p>
          Example:
          <code>/home/superadmin/servers/minecraft/start.sh</code>
          and
          <code>/home/superadmin/servers/minecraft/stop.sh</code>
        </p>
      </div>
    {:else}
      <div class="servers-grid" data-testid="servers-list">
        {#each filteredServers as server (server.id)}
          <article class:active={selectedServerId === server.id} class="server-card">
            <div
              role="button"
              tabindex="0"
              class="server-card-select"
              data-testid={`server-card-${slugify(server.name)}`}
              on:click={() => selectServer(server.id)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  void selectServer(server.id);
                }
              }}
            >
              <div class="server-card-head">
                <div>
                  <strong>{server.name}</strong>
                  <p>{server.description ?? 'No description yet.'}</p>
                </div>
                <span class={`status-pill ${statusTone(server.status.state)}`}>{statusLabel(server.status.state)}</span>
              </div>
            </div>
            {#if hasAnyActions(server)}
              <div class="server-card-actions">
                <button
                  type="button"
                  disabled={!server.start_script}
                  title={server.start_script ? 'Start server' : 'Configure actions to enable start'}
                  on:click|stopPropagation={() => runAction('start', server.id)}
                >
                  Start
                </button>
                <button
                  type="button"
                  disabled={!server.stop_script}
                  title={server.stop_script ? 'Stop server' : 'Configure actions to enable stop'}
                  on:click|stopPropagation={() => runAction('stop', server.id)}
                >
                  Stop
                </button>
                <button
                  type="button"
                  disabled={!server.restart_script}
                  title={server.restart_script ? 'Restart server' : 'Configure actions to enable restart'}
                  on:click|stopPropagation={() => runAction('restart', server.id)}
                >
                  Restart
                </button>
              </div>
            {:else}
              <p class="server-card-empty">No actions configured</p>
            {/if}
          </article>
        {/each}
      </div>
    {/if}
  </div>

  <section class="servers-detail-panel" data-testid="server-detail-panel">
    {#if selectedServer}
      <div class="server-context">
        <div>
          <p class="eyebrow">Selected server</p>
          <div class="server-context-title">
            <h2>{selectedServer.name}</h2>
            <span class={`status-pill ${statusTone(selectedServer.status.state)}`}>
              {statusLabel(selectedServer.status.state)}
            </span>
          </div>
          <p>{selectedServer.description ?? 'No description provided.'}</p>
        </div>
        <div class="server-context-actions">
          <button type="button" on:click={openEditForm}>Edit details</button>
          <button type="button" class="danger-button" on:click={openDeleteDialog}>Delete</button>
        </div>
      </div>

      {#if detailError}
        <div class="notice error">{detailError}</div>
      {/if}
      {#if detailLoading}
        <div class="panel-placeholder">Loading server details...</div>
      {/if}
      {#if actionError}
        <div class="notice error" data-testid="server-action-error">{actionError}</div>
      {/if}

      <section class="server-section">
        <div class="panel-head compact-head">
          <div>
            <h3>Actions</h3>
            <p>Start, stop, and restart script files for this server card.</p>
          </div>
          <button type="button" on:click={openActionsForm}>Configure actions</button>
        </div>

        {#if hasAnyActions(selectedServer)}
          <div class="server-script-grid">
            <article>
              <span>Working directory</span>
              <strong>{selectedServer.working_dir ?? 'Not configured'}</strong>
            </article>
            <article>
              <span>Start script</span>
              <strong>{selectedServer.start_script ?? 'Not configured'}</strong>
            </article>
            <article>
              <span>Stop script</span>
              <strong>{selectedServer.stop_script ?? 'Not configured'}</strong>
            </article>
            <article>
              <span>Restart script</span>
              <strong>{selectedServer.restart_script ?? 'Not configured'}</strong>
            </article>
          </div>

          <div class="context-actions">
            <button
              type="button"
              data-testid="server-action-start"
              disabled={actionLoading !== null || !selectedServer.start_script}
              title={actionHelpText('start')}
              on:click={() => runAction('start')}
            >
              {actionLabel('start')}
            </button>
            <button
              type="button"
              data-testid="server-action-stop"
              disabled={actionLoading !== null || !selectedServer.stop_script}
              title={actionHelpText('stop')}
              on:click={() => runAction('stop')}
            >
              {actionLabel('stop')}
            </button>
            <button
              type="button"
              data-testid="server-action-restart"
              disabled={actionLoading !== null || !selectedServer.restart_script}
              title={actionHelpText('restart')}
              on:click={() => runAction('restart')}
            >
              {actionLabel('restart')}
            </button>
          </div>

          {#if lastActionResult}
            <div class="action-output-inline" data-testid="server-action-output">
              <div class="output-summary">
                <div>
                  <span>Action</span>
                  <strong>{lastActionResult.action}</strong>
                </div>
                <div>
                  <span>Exit code</span>
                  <strong>{lastActionResult.exit_code ?? 'n/a'}</strong>
                </div>
                <div>
                  <span>Result</span>
                  <strong>{lastActionResult.ok ? 'ok' : 'failed'}</strong>
                </div>
              </div>
              <div class="output-blocks">
                <article>
                  <span>stdout</span>
                  <pre>{lastActionResult.stdout || 'No stdout output.'}</pre>
                </article>
                <article>
                  <span>stderr</span>
                  <pre>{lastActionResult.stderr || 'No stderr output.'}</pre>
                </article>
              </div>
            </div>
          {/if}
        {:else}
          <div class="panel-placeholder server-empty-state" data-testid="server-actions-empty">
            <div>
              <strong>No actions configured</strong>
              <p>Add start, stop, or restart script paths to enable server actions.</p>
            </div>
          </div>
        {/if}
      </section>

      <section class="server-section server-logs-section">
        <div class="panel-head compact-head">
          <div>
            <h3>Recent logs</h3>
            <p>{selectedServer.log_type ? 'Configured file or journal output for this server card.' : 'No logs configured'}</p>
          </div>
          {#if logsLoading}
            <span class="mini-status">Loading...</span>
          {/if}
          <button type="button" on:click={openLogsConfigForm}>Configure logs</button>
        </div>

        {#if selectedServer.log_type}
          <div class="server-logs-toolbar" data-testid="server-logs-toolbar">
            <button
              type="button"
              data-testid="server-logs-refresh"
              on:click={() => selectedServerId && loadLogs(selectedServerId)}
              disabled={!selectedServerId || logsLoading}
            >
              Refresh logs
            </button>

            <label class="server-logs-field" for="server-log-lines">
              <span>Lines</span>
              <select
                id="server-log-lines"
                data-testid="server-logs-lines"
                bind:value={logsLines}
                disabled={!selectedServerId || logsLoading}
                on:change={handleLogLineChange}
              >
                {#each logLineOptions as option (option)}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            </label>

            <label class="server-logs-field server-logs-search" for="server-log-search">
              <span class="sr-only">Filter logs</span>
              <input
                id="server-log-search"
                data-testid="server-logs-search"
                type="search"
                placeholder="Filter visible lines"
                bind:value={logsSearch}
              />
            </label>

            <label class="server-logs-toggle">
              <input type="checkbox" bind:checked={logsAutoRefresh} data-testid="server-logs-autorefresh" />
              <span>Auto-refresh</span>
            </label>

            <button type="button" data-testid="server-logs-copy" on:click={copyVisibleLogs}>
              Copy visible logs
            </button>
          </div>

          <div class="server-logs-meta">
            <span data-testid="server-logs-summary">Showing {filteredLogs.length} of {logs.length} lines</span>
            {#if logsAutoRefresh}
              <span>Refreshing every 3s</span>
            {/if}
            {#if logsCopyFeedback}
              <span>{logsCopyFeedback}</span>
            {/if}
          </div>

          {#if logsError}
            <div class="notice error" data-testid="server-logs-error">{logsError}</div>
          {/if}

          <pre class="server-logs" data-testid="server-logs">
{#if logs.length === 0}
No recent log lines.
{:else if filteredLogs.length === 0}
No visible log lines match the current filter.
{:else}
{filteredLogs.join('\n')}
{/if}</pre>
        {:else}
          <div class="panel-placeholder server-empty-state" data-testid="server-logs-empty">
            <div>
              <strong>No logs configured</strong>
              <p>Add a file path or journal unit to view recent logs here.</p>
            </div>
          </div>
        {/if}
      </section>

      {#if hasStatusConfigured(selectedServer)}
        <section class="server-section">
          <div class="panel-head compact-head">
            <div>
              <h3>Status</h3>
              <p>Configured status checks and the current state.</p>
            </div>
            <button type="button" on:click={openStatusConfigForm}>Configure status</button>
          </div>

          <div class="output-summary">
            <div>
              <span>Type</span>
              <strong>{selectedServer.status_type ?? 'manual'}</strong>
            </div>
            <div>
              <span>Configured value</span>
              <strong>{selectedServer.status_value ?? 'None'}</strong>
            </div>
            <div>
              <span>Current</span>
              <strong>{statusLabel(selectedServer.status.state)}</strong>
            </div>
          </div>

          <div class="panel-placeholder">{selectedServer.status.detail ?? 'Manual / unknown'}</div>
        </section>
      {/if}
    {:else}
      <div class="servers-empty detail-empty">
        <p class="eyebrow">Servers</p>
        <h3>Select a server card to inspect its scripts, action output, logs, and status.</h3>
      </div>
    {/if}
  </section>

  {#if metadataOpen}
    <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeMetadataForm}></button>
    <form
      class="dialog server-form-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-form-title"
      on:submit|preventDefault={submitMetadataForm}
    >
      <div class="dialog-head">
        <div>
          <p class="eyebrow">{metadataMode === 'create' ? 'Add server' : 'Edit server'}</p>
          <h2 id="server-form-title">
            {metadataMode === 'create' ? 'Create a server card' : `Edit ${metadataState.name}`}
          </h2>
          <p>
            {metadataMode === 'create'
              ? 'Create the card first, then configure scripts, logs, and status from the detail panel.'
              : 'Update the server card name and description.'}
          </p>
        </div>
        <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeMetadataForm}>x</button>
      </div>

      <div class="form-grid">
        <label class="full">
          <span>Name</span>
          <input data-testid="server-form-name" bind:value={metadataState.name} required />
        </label>
        <label class="full">
          <span>Description</span>
          <input data-testid="server-form-description" bind:value={metadataState.description} />
        </label>
      </div>

      {#if metadataError}
        <div class="notice error" role="alert">{metadataError}</div>
      {/if}

      <div class="dialog-actions">
        <button type="button" on:click={closeMetadataForm} disabled={metadataLoading}>Cancel</button>
        <button class="primary-button" type="submit" disabled={metadataLoading} data-testid="server-form-submit">
          {metadataLoading ? 'Saving...' : metadataMode === 'create' ? 'Create server' : 'Save changes'}
        </button>
      </div>
    </form>
  {/if}

  {#if actionsOpen}
    <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeActionsForm}></button>
    <form
      class="dialog server-form-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-actions-title"
      on:submit|preventDefault={submitActionsForm}
    >
      <div class="dialog-head">
        <div>
          <p class="eyebrow">Configure actions</p>
          <h2 id="server-actions-title">Edit action scripts</h2>
          <p>Pick existing script files from disk. HomePanel only stores paths.</p>
        </div>
        <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeActionsForm}>x</button>
      </div>

      <div class="form-grid">
        <label class="full">
          <span>Working directory</span>
          <div class="picker-field">
            <input data-testid="server-actions-working-dir" bind:value={actionsState.working_dir} />
            <button type="button" data-testid="server-actions-browse-working-dir" on:click={() => openPathPicker('working_dir', 'directory')}>Browse</button>
          </div>
        </label>
        <label class="full">
          <span>Start script</span>
          <div class="picker-field">
            <input data-testid="server-actions-start-script" bind:value={actionsState.start_script} />
            <button type="button" data-testid="server-actions-browse-start-script" on:click={() => openPathPicker('start_script', 'file')}>Browse</button>
          </div>
        </label>
        <label class="full">
          <span>Stop script</span>
          <div class="picker-field">
            <input data-testid="server-actions-stop-script" bind:value={actionsState.stop_script} />
            <button type="button" data-testid="server-actions-browse-stop-script" on:click={() => openPathPicker('stop_script', 'file')}>Browse</button>
          </div>
        </label>
        <label class="full">
          <span>Restart script</span>
          <div class="picker-field">
            <input data-testid="server-actions-restart-script" bind:value={actionsState.restart_script} />
            <button type="button" data-testid="server-actions-browse-restart-script" on:click={() => openPathPicker('restart_script', 'file')}>Browse</button>
          </div>
        </label>
      </div>

      {#if actionsError}
        <div class="notice error" role="alert">{actionsError}</div>
      {/if}

      <div class="dialog-actions">
        <button type="button" on:click={closeActionsForm} disabled={actionsLoading}>Cancel</button>
        <button class="primary-button" type="submit" disabled={actionsLoading} data-testid="server-actions-submit">
          {actionsLoading ? 'Saving...' : 'Save actions'}
        </button>
      </div>
    </form>
  {/if}

  {#if logsConfigOpen}
    <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeLogsConfigForm}></button>
    <form
      class="dialog server-form-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-logs-config-title"
      on:submit|preventDefault={submitLogsConfigForm}
    >
      <div class="dialog-head">
        <div>
          <p class="eyebrow">Configure logs</p>
          <h2 id="server-logs-config-title">Edit log source</h2>
          <p>Choose file or journal logs. A null source keeps the logs panel empty.</p>
        </div>
        <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeLogsConfigForm}>x</button>
      </div>

      <div class="form-grid">
        <label class="full">
          <span>Log source</span>
          <select bind:value={logsConfigState.log_mode} data-testid="server-logs-log-type">
            <option value="none">None</option>
            <option value="file">File</option>
            <option value="journal">Journal</option>
          </select>
        </label>
        {#if logsConfigState.log_mode === 'file'}
          <label class="full">
            <span>Log file path</span>
            <div class="picker-field">
              <input data-testid="server-logs-log-path" bind:value={logsConfigState.log_path} placeholder="/srv/logs/minecraft/latest.log" />
              <button type="button" data-testid="server-logs-browse-log-path" on:click={() => openPathPicker('log_path', 'file')}>Browse</button>
            </div>
          </label>
        {:else if logsConfigState.log_mode === 'journal'}
          <label class="full">
            <span>Journal unit</span>
            <input data-testid="server-logs-log-unit" bind:value={logsConfigState.log_unit} placeholder="minecraft.service" />
          </label>
        {/if}
      </div>

      {#if logsConfigError}
        <div class="notice error" role="alert">{logsConfigError}</div>
      {/if}

      <div class="dialog-actions">
        <button type="button" on:click={closeLogsConfigForm} disabled={logsConfigLoading}>Cancel</button>
        <button class="primary-button" type="submit" disabled={logsConfigLoading} data-testid="server-logs-submit">
          {logsConfigLoading ? 'Saving...' : 'Save logs'}
        </button>
      </div>
    </form>
  {/if}

  {#if statusConfigOpen}
    <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeStatusConfigForm}></button>
    <form
      class="dialog server-form-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-status-config-title"
      on:submit|preventDefault={submitStatusConfigForm}
    >
      <div class="dialog-head">
        <div>
          <p class="eyebrow">Configure status</p>
          <h2 id="server-status-config-title">Edit status check</h2>
          <p>Status checks are optional. Manual means HomePanel only shows the stored state.</p>
        </div>
        <button type="button" class="icon-button" aria-label="Close dialog" on:click={closeStatusConfigForm}>x</button>
      </div>

      <div class="form-grid">
        <label class="full">
          <span>Status type</span>
          <select data-testid="server-status-type" bind:value={statusConfigState.status_type}>
            {#each statusOptions as option (option.value)}
              <option value={option.value}>{option.label}</option>
            {/each}
          </select>
        </label>
        {#if statusConfigState.status_type !== 'manual'}
          <label class="full">
            <span>Status value</span>
            <input
              data-testid="server-status-value"
              bind:value={statusConfigState.status_value}
              placeholder={statusConfigState.status_type === 'systemd' ? 'minecraft.service' : 'java'}
            />
          </label>
        {/if}
      </div>

      {#if statusConfigError}
        <div class="notice error" role="alert">{statusConfigError}</div>
      {/if}

      <div class="dialog-actions">
        <button type="button" on:click={closeStatusConfigForm} disabled={statusConfigLoading}>Cancel</button>
        <button class="primary-button" type="submit" disabled={statusConfigLoading} data-testid="server-status-submit">
          {statusConfigLoading ? 'Saving...' : 'Save status'}
        </button>
      </div>
    </form>
  {/if}

  {#if pathPicker}
    <PathPickerDialog
      open={true}
      title={pathPicker.field === 'working_dir' ? 'Choose working directory' : 'Choose script path'}
      description={pathPicker.field === 'working_dir'
        ? 'Browse a directory and choose the folder HomePanel should use.'
        : 'Browse to an executable script file.'}
      mode={pathPicker.mode}
      confirmLabel={pathPicker.mode === 'directory' ? 'Use folder' : 'Use file'}
      initialPath={
        pathPicker.field === 'working_dir'
          ? actionsState.working_dir
          : pathPicker.field === 'start_script'
            ? actionsState.start_script
            : pathPicker.field === 'stop_script'
              ? actionsState.stop_script
              : pathPicker.field === 'restart_script'
                ? actionsState.restart_script
                : logsConfigState.log_path
      }
      onSelect={applyPickedPath}
      onClose={closePathPicker}
    />
  {/if}

  {#if deleteOpen && selectedServer}
    <button type="button" class="dialog-backdrop" aria-label="Close dialog" on:click={closeDeleteDialog}></button>
    <form
      class="dialog server-delete-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="server-delete-title"
      on:submit|preventDefault={confirmDelete}
    >
      <div class="dialog-head">
        <div>
          <p class="eyebrow">Delete server</p>
          <h2 id="server-delete-title">Delete {selectedServer.name}?</h2>
          <p>This removes the card from HomePanel. It does not delete any scripts or files on disk.</p>
        </div>
      </div>

      {#if deleteError}
        <div class="notice error" role="alert">{deleteError}</div>
      {/if}

      <div class="dialog-actions">
        <button type="button" on:click={closeDeleteDialog} disabled={deleteLoading}>Cancel</button>
        <button class="primary-button danger-button" type="submit" disabled={deleteLoading} data-testid="server-delete-confirm">
          {deleteLoading ? 'Deleting...' : 'Delete server'}
        </button>
      </div>
    </form>
  {/if}
</section>

<style>
  .servers-page {
    min-height: 0;
    height: 100%;
    overflow: hidden;
    display: grid;
    grid-template-columns: 380px minmax(0, 1fr);
  }

  .servers-list-panel,
  .servers-detail-panel {
    min-width: 0;
    min-height: 0;
    padding: 16px;
  }

  .servers-list-panel {
    overflow: hidden;
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr);
    gap: 12px;
    background: #f2f5f7;
    border-right: 1px solid var(--line);
  }

  .servers-detail-panel {
    overflow: auto;
    display: grid;
    align-content: start;
    gap: 12px;
  }

  .servers-toolbar {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface-strong);
  }

  .servers-grid {
    min-height: 0;
    overflow: auto;
    display: grid;
    align-content: start;
    gap: 10px;
    padding-right: 6px;
  }

  .server-card {
    display: grid;
    gap: 12px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--surface-strong);
    box-shadow: 0 1px rgba(32, 38, 45, 0.04);
    text-align: left;
  }

  .server-card-select {
    display: block;
    border-radius: 10px;
  }

  .server-card.active {
    border-color: var(--accent);
    box-shadow: inset 0 0 0 1px var(--accent);
    background: #eef3f7;
  }

  .server-card-head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }

  .server-card-head strong,
  .server-context h2,
  .panel-head h3,
  .servers-empty h3 {
    margin: 0;
  }

  .server-card-head p,
  .server-context p,
  .panel-head p,
  .servers-empty p {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 0.84rem;
  }

  .server-card-actions,
  .server-context-actions,
  .context-actions,
  .dialog-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .server-card-actions {
    margin-top: 2px;
  }

  .server-context-actions {
    align-self: flex-start;
    margin-top: 2px;
  }

  .panel-head.compact-head {
    gap: 10px;
    padding-bottom: 2px;
  }

  .server-context,
  .server-section,
  .server-output-panel,
  .server-logs-panel {
    border: 1px solid var(--line);
    border-radius: 14px;
    background: var(--surface-strong);
  }

  .server-context,
  .server-output-panel,
  .server-logs-panel,
  .server-section {
    padding: 18px;
  }

  .server-section {
    display: grid;
    gap: 14px;
  }

  .server-context {
    display: flex;
    justify-content: space-between;
    gap: 14px;
    align-items: flex-start;
  }

  .server-context-title {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .server-script-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin-top: 8px;
  }

  .server-script-grid article {
    display: grid;
    gap: 6px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface);
  }

  .server-script-grid span,
  .output-summary span,
  .output-blocks span,
  .form-grid span {
    color: var(--muted);
    font-size: 0.74rem;
    font-weight: 700;
    text-transform: uppercase;
  }

  .server-script-grid strong,
  .output-summary strong {
    font-size: 0.9rem;
    overflow-wrap: anywhere;
  }

  .server-script-grid strong {
    line-height: 1.45;
  }

  .output-summary {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
    margin-top: 10px;
  }

  .action-output-inline {
    display: grid;
    gap: 10px;
    margin-top: 14px;
  }

  .output-summary div,
  .output-blocks article {
    display: grid;
    gap: 6px;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--surface);
  }

  .output-blocks {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
    margin-top: 10px;
  }

  .output-blocks pre,
  .server-logs {
    margin: 0;
    padding: 12px;
    border-radius: 12px;
    background: var(--terminal);
    color: #d8dee5;
    font-family: var(--mono);
    font-size: 0.78rem;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .server-logs-toolbar {
    display: grid;
    grid-template-columns: auto auto minmax(180px, 1fr) auto auto;
    gap: 8px;
    align-items: center;
    margin-top: 12px;
  }

  .server-logs-field,
  .server-logs-toggle {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .server-logs-search input {
    width: 100%;
  }

  .server-logs-meta {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    margin: 12px 0;
    color: var(--muted);
    font-size: 0.78rem;
  }

  .servers-empty,
  .panel-placeholder {
    padding: 18px;
    border: 1px dashed var(--line-strong);
    border-radius: 14px;
    background: var(--surface);
    text-align: center;
  }

  .detail-empty {
    min-height: 320px;
    display: grid;
    place-items: center;
    align-content: center;
  }

  .servers-empty code {
    display: inline-block;
    margin: 4px 4px 0;
    font-family: var(--mono);
  }

  .server-empty-state {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
    text-align: left;
    padding-block: 20px;
  }

  .server-empty-state p,
  .server-card-empty {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 0.84rem;
  }

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
    max-height: calc(100vh - 40px);
    overflow: auto;
    transform: translate(-50%, -50%);
    display: grid;
    gap: 12px;
    padding: 16px;
    border: 1px solid var(--line-strong);
    border-radius: 14px;
    background: var(--surface-strong);
    box-shadow: 0 20px 52px rgba(32, 38, 45, 0.2);
  }

  .dialog-head {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: flex-start;
    padding-bottom: 6px;
    border-bottom: 1px solid var(--line);
  }

  .form-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .form-grid label {
    display: grid;
    gap: 6px;
  }

  .form-grid label.full {
    grid-column: 1 / -1;
  }

  .picker-field {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 8px;
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

  .servers-search-label {
    display: block;
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

  .mini-status {
    color: var(--muted);
    font-size: 0.76rem;
    text-transform: uppercase;
  }

  @media (max-width: 980px) {
    .servers-page {
      grid-template-columns: 1fr;
    }

    .servers-list-panel {
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }

    .server-logs-toolbar {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .server-logs-search {
      grid-column: 1 / -1;
    }
  }

  @media (max-width: 720px) {
    .server-context,
    .output-summary,
    .output-blocks,
    .server-script-grid,
    .form-grid,
    .server-logs-toolbar,
    .dialog-actions,
    .picker-field {
      grid-template-columns: 1fr;
      display: grid;
    }

    .server-context,
    .server-context-actions,
    .context-actions {
      display: grid;
    }
  }
</style>
