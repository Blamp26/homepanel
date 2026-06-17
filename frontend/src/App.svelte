<script lang="ts">
  import { onMount } from 'svelte';
  import NewTerminalDialog from './components/terminal/NewTerminalDialog.svelte';
  import TerminalTabs from './components/terminal/TerminalTabs.svelte';
  import TerminalView from './components/terminal/TerminalView.svelte';
  import {
    clearTerminalScrollback,
    createTerminal,
    killTerminal,
    listTerminals,
    type TerminalSummary,
  } from './api/terminals';
  import {
    getService,
    getServiceLogs,
    listServices,
    restartService,
    startService,
    stopService,
    type ServiceDetails,
    type ServiceSummary,
  } from './api/services';
  import { login, logout, setup, status as authStatus } from './api/auth';

  type Page =
    | 'dashboard'
    | 'terminals'
    | 'files'
    | 'services'
    | 'logs'
    | 'game servers'
    | 'settings';

  const pages: Array<{ value: Page; label: string; state: 'ready' | 'soon' }> =
    [
      { value: 'terminals', label: 'Terminals', state: 'ready' },
      { value: 'dashboard', label: 'Overview', state: 'ready' },
      { value: 'files', label: 'Files', state: 'soon' },
      { value: 'services', label: 'Services', state: 'ready' },
      { value: 'logs', label: 'Logs', state: 'soon' },
      { value: 'game servers', label: 'Game servers', state: 'soon' },
      { value: 'settings', label: 'Settings', state: 'soon' },
    ];

  let user: string | null = null;
  let loading = true;
  let authMode: 'login' | 'setup' = 'login';
  let username = '';
  let password = '';
  let page: Page = 'terminals';
  let terminals: TerminalSummary[] = [];
  let activeTerminalId: string | null = null;
  let showNewTerminal = false;
  let terminalClearToken = 0;
  let error = '';
  let terminalError = '';
  let killingTerminalId: string | null = null;
  let services: ServiceSummary[] = [];
  let servicesLoading = false;
  let servicesError = '';
  let serviceDetails: ServiceDetails | null = null;
  let serviceDetailsLoading = false;
  let serviceLogs: string[] = [];
  let serviceLogsLoading = false;
  let serviceActionLoading: 'start' | 'stop' | 'restart' | null = null;
  let serviceActionError = '';
  let serviceError = '';
  let selectedServiceName: string | null = null;

  async function refresh() {
    terminalError = '';
    terminals = await listTerminals();
    activeTerminalId = pickActiveTerminalId(terminals, activeTerminalId);
  }

  function updateTerminalStatus(terminalId: string, status: string) {
    terminals = terminals.map((terminal) =>
      terminal.id === terminalId ? { ...terminal, status } : terminal,
    );
  }

  function handleTerminalStatusChange(terminalId: string, status: string) {
    updateTerminalStatus(terminalId, status);
    if (
      terminalId === activeTerminalId &&
      !['failed', 'exited'].includes(status.toLowerCase())
    ) {
      terminalError = '';
    }
  }

  function handleTerminalAttachError(message: string) {
    terminalError = message;
  }

  function serviceTone(value: string | undefined) {
    const normalized = value?.toLowerCase() ?? '';
    if (normalized.includes('active') || normalized.includes('running'))
      return 'good';
    if (
      normalized.includes('failed') ||
      normalized.includes('inactive') ||
      normalized.includes('dead')
    )
      return 'bad';
    return 'neutral';
  }

  function isSelectableTerminal(status: string | undefined) {
    const value = status?.toLowerCase() ?? '';
    return value !== 'exited' && value !== 'failed';
  }

  function pickActiveTerminalId(
    nextTerminals: TerminalSummary[],
    preferredId: string | null,
  ) {
    const preferred = nextTerminals.find((terminal) => terminal.id === preferredId);
    if (preferred && isSelectableTerminal(preferred.status)) {
      return preferred.id;
    }

    return (
      nextTerminals.find((terminal) => isSelectableTerminal(terminal.status))
        ?.id ?? null
    );
  }

  function serviceTestId(name: string) {
    return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '');
  }

  async function loadSelectedService(name: string) {
    serviceDetailsLoading = true;
    serviceLogsLoading = true;
    serviceError = '';
    try {
      const [details, logs] = await Promise.all([
        getService(name),
        getServiceLogs(name),
      ]);
      serviceDetails = details;
      serviceLogs = logs.items;
    } catch (err) {
      serviceError = err instanceof Error ? err.message : String(err);
      serviceDetails = null;
      serviceLogs = [];
    } finally {
      serviceDetailsLoading = false;
      serviceLogsLoading = false;
    }
  }

  async function loadServices() {
    if (servicesLoading) return;
    servicesLoading = true;
    servicesError = '';
    try {
      const response = await listServices();
      services = response.items;
      const preferred = selectedServiceName
        ? response.items.find((service) => service.name === selectedServiceName)
        : null;
      selectedServiceName = preferred?.name ?? response.items[0]?.name ?? null;
      if (selectedServiceName) {
        await loadSelectedService(selectedServiceName);
      } else {
        serviceDetails = null;
        serviceLogs = [];
      }
    } catch (err) {
      servicesError = err instanceof Error ? err.message : String(err);
      services = [];
      selectedServiceName = null;
      serviceDetails = null;
      serviceLogs = [];
    } finally {
      servicesLoading = false;
    }
  }

  async function selectPage(next: Page) {
    page = next;
    if (next === 'services') {
      await loadServices();
    }
  }

  async function selectService(name: string) {
    selectedServiceName = name;
    await loadSelectedService(name);
  }

  async function runServiceAction(action: 'start' | 'stop' | 'restart') {
    if (!selectedServiceName) return;

    serviceActionLoading = action;
    serviceActionError = '';
    try {
      if (action === 'start') {
        await startService(selectedServiceName);
      } else if (action === 'stop') {
        await stopService(selectedServiceName);
      } else {
        await restartService(selectedServiceName);
      }
      await loadServices();
    } catch (err) {
      serviceActionError = err instanceof Error ? err.message : String(err);
    } finally {
      serviceActionLoading = null;
    }
  }

  async function syncAuthState() {
    try {
      const response = await authStatus();
      authMode = response.setup_required ? 'setup' : 'login';
      user = response.authenticated ? response.username : null;
      if (user) {
        await refresh();
      }
    } catch {
      authMode = 'login';
      user = null;
    }
  }

  onMount(async () => {
    try {
      await syncAuthState();
    } finally {
      loading = false;
    }
  });

  async function submitAuth() {
    error = '';
    try {
      if (authMode === 'setup') {
        await setup(username, password);
      } else {
        await login(username, password);
      }
      username = '';
      password = '';
      await syncAuthState();
    } catch (err) {
      error = err instanceof Error ? err.message : String(err);
    }
  }

  async function createTerminalFromDialog(payload: Record<string, unknown>) {
    terminalError = '';
    try {
      const terminal = await createTerminal(payload);
      showNewTerminal = false;
      await refresh();
      activeTerminalId = terminal.id;
    } catch (err) {
      terminalError = err instanceof Error ? err.message : String(err);
    }
  }

  async function killTerminalById(terminalId: string) {
    const target = terminals.find((terminal) => terminal.id === terminalId);
    if (!target) return;

    terminalError = '';
    if (activeTerminalId === terminalId) {
      killingTerminalId = terminalId;
    }

    try {
      await killTerminal(terminalId);
      updateTerminalStatus(terminalId, 'exited');
      await refresh();
    } catch (err) {
      terminalError = err instanceof Error ? err.message : String(err);
    } finally {
      if (killingTerminalId === terminalId) {
        killingTerminalId = null;
      }
    }
  }

  async function clearActiveTerminal() {
    if (!activeTerminal) return;
    terminalError = '';
    try {
      await clearTerminalScrollback(activeTerminal.id);
      terminalClearToken += 1;
    } catch (err) {
      terminalError = err instanceof Error ? err.message : String(err);
    }
  }

  async function refreshAll() {
    await refresh();
  }

  async function signOut() {
    await logout();
    username = '';
    password = '';
    user = null;
    terminals = [];
    activeTerminalId = null;
    services = [];
    selectedServiceName = null;
    serviceDetails = null;
    serviceLogs = [];
    servicesError = '';
    serviceError = '';
    await syncAuthState();
  }

  function shortId(id: string) {
    return id.slice(0, 8);
  }

  function statusTone(status: string | undefined) {
    const value = status?.toLowerCase() ?? '';
    if (value.includes('run')) return 'good';
    if (
      value.includes('exit') ||
      value.includes('dead') ||
      value.includes('stop')
    )
      return 'bad';
    return 'neutral';
  }

  $: activeTerminal =
    terminals.find((terminal) => terminal.id === activeTerminalId) ?? null;
  $: visibleTerminals = terminals.filter((terminal) =>
    isSelectableTerminal(terminal.status),
  );
  $: activeService =
    services.find((service) => service.name === selectedServiceName) ?? null;
  $: pageTitle =
    pages.find((item) => item.value === page)?.label ?? 'HomePanel';
</script>

{#if loading}
  <main class="auth-screen">
    <div class="auth-card loading-card">Checking your session...</div>
  </main>
{:else if !user}
  <main class="auth-screen">
    <form class="auth-card" on:submit|preventDefault={submitAuth}>
      <div class="auth-brand">
        <span class="brand-mark">HP</span>
        <div>
          <h1>HomePanel</h1>
          <p>
            {authMode === 'setup'
              ? 'Initial setup'
              : 'Sign in to manage this host.'}
          </p>
        </div>
      </div>

      <p class="auth-copy">
        {authMode === 'setup'
          ? 'Create the first user for this host.'
          : 'Use your existing account to continue.'}
      </p>

      <label for="login-user">Username</label>
      <input
        id="login-user"
        bind:value={username}
        autocomplete="username"
        placeholder={authMode === 'setup' ? 'Choose a username' : 'Username'}
      />

      <label for="login-pass">Password</label>
      <input
        id="login-pass"
        type="password"
        bind:value={password}
        autocomplete="current-password"
      />

      {#if error}
        <div class="notice error">{error}</div>
      {/if}

      <button class="primary-button" type="submit"
        >{authMode === 'setup' ? 'Create first user' : 'Sign in'}</button
      >
    </form>
  </main>
{:else}
  <div class="app-shell">
    <aside class="side-rail">
      <div class="product-lockup">
        <span class="brand-mark">HP</span>
        <div>
          <strong>HomePanel</strong>
          <span>{user}</span>
        </div>
      </div>

      <nav class="primary-nav" aria-label="Primary">
        {#each pages as item (item.value)}
          <button
            type="button"
            class:active={page === item.value}
            on:click={() => selectPage(item.value)}
          >
            <span>{item.label}</span>
            {#if item.state === 'soon'}
              <small>soon</small>
            {:else if item.value === 'terminals'}
              <small>{visibleTerminals.length}</small>
            {/if}
          </button>
        {/each}
      </nav>

      <div class="rail-footer">
        <div class="connection-status">
          <span class="status-dot good"></span>
          <span>Connected</span>
        </div>
        <button class="quiet-button" type="button" on:click={signOut}
          >Logout</button
        >
      </div>
    </aside>

    <main class="workspace">
      <header class="workspace-header">
        <div>
          <p class="eyebrow">Host admin</p>
          <h1>{pageTitle}</h1>
        </div>
      </header>

      {#if page === 'terminals'}
        <section class="terminal-page" aria-label="Terminal sessions">
          <div class="session-panel">
            <div class="panel-head">
              <div>
                <h2>Sessions</h2>
                <p>Persistent browser terminals</p>
              </div>
              <button
                class="primary-button compact"
                type="button"
                on:click={() => (showNewTerminal = true)}>New</button
              >
            </div>

            <TerminalTabs
              terminals={visibleTerminals}
              activeId={activeTerminalId}
              onSelect={(id) => (activeTerminalId = id)}
              onClose={killTerminalById}
            />

            <button class="wide-button" type="button" on:click={refreshAll}
              >Refresh sessions</button
            >
          </div>

          <section
            class="terminal-workbench"
            data-testid="active-terminal-panel"
          >
            {#if activeTerminal}
              {#key activeTerminal.id}
                <div class="terminal-context">
                  <div class="terminal-title">
                    <span
                      class={`status-dot ${statusTone(activeTerminal.status)}`}
                    ></span>
                    <div>
                      <h2>{activeTerminal.name}</h2>
                      <p>
                        <span data-testid="active-terminal-command"
                          >{activeTerminal.command}</span
                        >
                        ·
                        <span data-testid="active-terminal-cwd"
                          >{activeTerminal.cwd}</span
                        >
                      </p>
                    </div>
                  </div>

                  <dl class="terminal-facts">
                    <div>
                      <dt>Status</dt>
                      <dd data-testid="active-terminal-status">
                        {activeTerminal.status}
                      </dd>
                    </div>
                    <div>
                      <dt>ID</dt>
                      <dd>{shortId(activeTerminal.id)}</dd>
                    </div>
                    <div>
                      <dt>Size</dt>
                      <dd>{activeTerminal.cols}x{activeTerminal.rows}</dd>
                    </div>
                  </dl>

                  <div class="context-actions">
                    <button type="button" on:click={clearActiveTerminal}
                      >Clear</button
                    >
                  </div>
                </div>

                {#if terminalError}
                  <div class="notice error">{terminalError}</div>
                {/if}

                {#if activeTerminal.status.toLowerCase() === 'failed'}
                  <div class="empty-terminal">
                    <div>
                      <p class="eyebrow">Terminal unavailable</p>
                      <h2>Terminal process is no longer available.</h2>
                      <p>Create a new terminal or pick another live session.</p>
                    </div>
                    <button
                      class="primary-button"
                      type="button"
                      on:click={() => (showNewTerminal = true)}
                      >New /bin/bash terminal</button
                    >
                  </div>
                {:else}
                  <div class="terminal-frame" data-testid="terminal-viewport">
                    <TerminalView
                      terminalId={activeTerminal.id}
                      terminalStatus={activeTerminal.status}
                      isKilling={killingTerminalId === activeTerminal.id}
                      clearToken={terminalClearToken}
                      onStatusChange={handleTerminalStatusChange.bind(
                        null,
                        activeTerminal.id,
                      )}
                      onAttachError={handleTerminalAttachError}
                    />
                  </div>
                {/if}
              {/key}
            {:else}
              <div class="empty-terminal">
                <div>
                  <p class="eyebrow">No session selected</p>
                  <h2>Start a shell on this host.</h2>
                  <p>
                    Create a persistent terminal session, then use it here.
                    Sessions remain available after refreshing the browser.
                  </p>
                </div>
                <button
                  class="primary-button"
                  type="button"
                  on:click={() => (showNewTerminal = true)}
                  >New /bin/bash terminal</button
                >
              </div>
            {/if}
          </section>
        </section>
      {:else if page === 'dashboard'}
        <section class="simple-page">
          <div class="summary-grid">
            <article>
              <span>Open terminals</span>
              <strong>{visibleTerminals.length}</strong>
            </article>
            <article>
              <span>Selected terminal</span>
              <strong>{activeTerminal?.name ?? 'None'}</strong>
            </article>
          </div>
        </section>
      {:else if page === 'services'}
        <section class="services-page" aria-label="Systemd services">
          <div class="service-panel">
            <div class="panel-head">
              <div>
                <h2>Services</h2>
                <p>Manage systemd units on this host</p>
              </div>
              <button
                class="primary-button compact"
                type="button"
                data-testid="services-refresh"
                on:click={loadServices}
              >
                Refresh
              </button>
            </div>

            {#if servicesLoading && services.length === 0}
              <div class="panel-placeholder">Loading services...</div>
            {:else if servicesError}
              <div class="notice error" role="alert">{servicesError}</div>
            {:else}
              <div class="service-list" data-testid="service-list">
                {#if services.length === 0}
                  <div class="panel-placeholder">No service units found.</div>
                {:else}
                  {#each services as service (service.name)}
                    <button
                      type="button"
                      class:active={selectedServiceName === service.name}
                      class="service-row"
                      data-testid={`service-row-${serviceTestId(service.name)}`}
                      on:click={() => selectService(service.name)}
                    >
                      <div class="service-row-top">
                        <span class="service-row-name">{service.name}</span>
                        <span
                          class={`service-pill ${serviceTone(service.active)}`}
                        >
                          {service.active} · {service.sub}
                        </span>
                      </div>
                      <p>{service.description}</p>
                    </button>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>

          <section class="service-workbench" data-testid="service-details-panel">
            {#if activeService}
              <div class="service-context">
                <div>
                  <p class="eyebrow">Selected service</p>
                  <h2>{activeService.name}</h2>
                  <p>{activeService.description}</p>
                </div>
                <dl class="service-facts">
                  <div>
                    <dt>Load</dt>
                    <dd>{serviceDetails?.load_state ?? activeService.load}</dd>
                  </div>
                  <div>
                    <dt>Active</dt>
                    <dd>{serviceDetails?.active_state ?? activeService.active}</dd>
                  </div>
                  <div>
                    <dt>Sub</dt>
                    <dd>{serviceDetails?.sub_state ?? activeService.sub}</dd>
                  </div>
                  <div>
                    <dt>Unit file</dt>
                    <dd>{serviceDetails?.unit_file_state ?? 'unknown'}</dd>
                  </div>
                </dl>
                <div class="context-actions">
                  <button
                    type="button"
                    disabled={serviceActionLoading === 'start'}
                    on:click={() => runServiceAction('start')}
                  >
                    Start
                  </button>
                  <button
                    type="button"
                    disabled={serviceActionLoading === 'stop'}
                    on:click={() => runServiceAction('stop')}
                  >
                    Stop
                  </button>
                  <button
                    type="button"
                    disabled={serviceActionLoading === 'restart'}
                    on:click={() => runServiceAction('restart')}
                  >
                    Restart
                  </button>
                </div>
              </div>

              {#if serviceActionError}
                <div class="notice error">{serviceActionError}</div>
              {/if}

              {#if serviceError}
                <div class="notice error">{serviceError}</div>
              {/if}

              {#if serviceDetailsLoading}
                <div class="panel-placeholder">Loading service details...</div>
              {/if}

              <div class="service-details-grid">
                <article>
                  <span>Fragment</span>
                  <strong>{serviceDetails?.fragment_path ?? 'n/a'}</strong>
                </article>
                <article>
                  <span>Main PID</span>
                  <strong>{serviceDetails?.main_pid ?? 0}</strong>
                </article>
                <article>
                  <span>Memory</span>
                  <strong>
                    {serviceDetails?.memory_current === null ||
                    serviceDetails?.memory_current === undefined
                      ? 'n/a'
                      : `${serviceDetails.memory_current} bytes`}
                  </strong>
                </article>
                <article>
                  <span>CPU</span>
                  <strong>
                    {serviceDetails?.cpu_usage_nsec === null ||
                    serviceDetails?.cpu_usage_nsec === undefined
                      ? 'n/a'
                      : `${serviceDetails.cpu_usage_nsec} ns`}
                  </strong>
                </article>
              </div>

              <section class="service-logs-panel">
                <div class="panel-head compact-head">
                  <div>
                    <h3>Recent logs</h3>
                    <p>Last 200 journal lines for this unit</p>
                  </div>
                  {#if serviceLogsLoading}
                    <span class="mini-status">Loading...</span>
                  {/if}
                </div>
                <pre data-testid="service-logs" class="service-logs">
{#if serviceLogs.length === 0}
No recent log lines.
{:else}
{serviceLogs.join('\n')}
{/if}</pre>
              </section>
            {:else}
              <div class="empty-page">
                <p class="eyebrow">Services</p>
                <h2>Select a service to inspect it.</h2>
                <p>
                  The first real Services page focuses on browsing units, reading
                  recent journal output, and starting or stopping the selected
                  service.
                </p>
              </div>
            {/if}
          </section>
        </section>
      {:else}
        <section class="simple-page">
          <div class="empty-page">
            <p class="eyebrow">{pageTitle}</p>
            <h2>This tool is not implemented yet.</h2>
            <p>
              Navigation is in place so future server tools have a clear home
              without competing with the terminal workflow.
            </p>
          </div>
        </section>
      {/if}
    </main>

    <NewTerminalDialog
      open={showNewTerminal}
      onCreate={createTerminalFromDialog}
      onClose={() => (showNewTerminal = false)}
    />
  </div>
{/if}
