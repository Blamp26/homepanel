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
      { value: 'services', label: 'Services', state: 'soon' },
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
            on:click={() => (page = item.value)}
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
