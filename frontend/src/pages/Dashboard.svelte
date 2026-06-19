<script lang="ts">
  import { onMount } from 'svelte';
  import { getOverview, type OverviewResponse } from '../api/overview';

  export let currentUser: string | null = null;
  export let onNavigate: (
    page: 'terminals' | 'files' | 'services' | 'logs',
  ) => void = () => {};

  let overview: OverviewResponse | null = null;
  let loading = true;
  let error = '';
  let requestToken = 0;
  let refreshing = false;
  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let requestInFlight = false;
  type OverviewDisk = OverviewResponse['disks'][number];

  function formatBytes(bytes: number | null | undefined) {
    if (bytes === null || bytes === undefined || Number.isNaN(bytes)) {
      return 'n/a';
    }

    const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
    let value = bytes;
    let unit = 0;

    while (value >= 1024 && unit < units.length - 1) {
      value /= 1024;
      unit += 1;
    }

    const digits = value >= 100 || unit === 0 ? 0 : value >= 10 ? 1 : 2;
    return `${value.toFixed(digits)} ${units[unit]}`;
  }

  function formatCpuUsage(percent: number | null | undefined) {
    if (percent === null || percent === undefined || Number.isNaN(percent)) {
      return 'n/a';
    }

    const digits = percent < 10 ? 1 : 0;
    return `${percent.toFixed(digits)}%`;
  }

  function formatUptime(seconds: number | null | undefined) {
    if (seconds === null || seconds === undefined || Number.isNaN(seconds)) {
      return 'n/a';
    }

    const total = Math.max(0, Math.floor(seconds));
    const days = Math.floor(total / 86_400);
    const hours = Math.floor((total % 86_400) / 3_600);
    const minutes = Math.floor((total % 3_600) / 60);

    const parts: string[] = [];
    if (days > 0) parts.push(`${days}d`);
    if (hours > 0 || days > 0) parts.push(`${hours}h`);
    parts.push(`${minutes}m`);
    return parts.join(' ');
  }

  function formatPercent(used: number | null | undefined, total: number | null | undefined) {
    if (
      used === null ||
      used === undefined ||
      total === null ||
      total === undefined ||
      total === 0
    ) {
      return 'n/a';
    }

    return `${Math.round((used / total) * 100)}%`;
  }

  function formatCapacity(used: number | null | undefined, total: number | null | undefined) {
    const usedText = formatBytes(used);
    const totalText = formatBytes(total);
    if (usedText === 'n/a' || totalText === 'n/a') {
      return 'n/a';
    }
    return `${usedText} / ${totalText}`;
  }

  function formatMemorySummary() {
    if (
      overview?.memory_used_bytes === null ||
      overview?.memory_used_bytes === undefined ||
      overview?.memory_total_bytes === null ||
      overview?.memory_total_bytes === undefined
    ) {
      return 'n/a';
    }

    return formatCapacity(overview.memory_used_bytes, overview.memory_total_bytes);
  }

  function sameDiskSignature(left: OverviewDisk | null, right: OverviewDisk | null) {
    return (
      !!left &&
      !!right &&
      left.total_bytes === right.total_bytes &&
      left.available_bytes === right.available_bytes &&
      left.used_bytes === right.used_bytes
    );
  }

  async function refreshOverview() {
    if (requestInFlight) return;

    const token = ++requestToken;
    requestInFlight = true;
    refreshing = true;
    if (!overview) {
      loading = true;
    }

    try {
      const response = await getOverview();
      if (token !== requestToken) return;
      overview = response;
      error = '';
    } catch (err) {
      if (token !== requestToken) return;
      if (!overview) {
        error = err instanceof Error ? err.message : String(err);
        overview = null;
      }
    } finally {
      if (token === requestToken) {
        loading = false;
        refreshing = false;
        requestInFlight = false;
      }
    }
  }

  function formatList(values: string[]) {
    return values.length > 0 ? values.join(', ') : 'n/a';
  }

  onMount(() => {
    void refreshOverview();
    pollTimer = setInterval(() => {
      void refreshOverview();
    }, 2_000);

    return () => {
      if (pollTimer !== null) {
        clearInterval(pollTimer);
        pollTimer = null;
      }
    };
  });

</script>

<section class="dashboard-page" aria-busy={loading} data-testid="dashboard-page">
  <header class="dashboard-header">
    <div class="dashboard-heading">
      <p class="eyebrow">Overview</p>
      <h2 data-testid="dashboard-hostname">{overview?.hostname ?? 'Server overview'}</h2>
      <p class="dashboard-subtitle" data-testid="dashboard-summary">
        {#if overview}
          {formatUptime(overview.uptime_seconds) === 'n/a'
            ? 'Uptime unavailable'
            : `Up ${formatUptime(overview.uptime_seconds)}`} · CPU
          {formatCpuUsage(overview.cpu_usage_percent)} · RAM
          {formatMemorySummary()} ·
          {formatList(overview.primary_ips)}
        {:else if loading}
          Loading server status...
        {:else}
          Status unavailable
        {/if}
      </p>
    </div>

    <div class="dashboard-header-meta">
      <span class="dashboard-pill">
        {error && !overview ? 'Offline' : refreshing && overview ? 'Refreshing' : overview ? 'Online' : 'Loading'}
      </span>
      <span class="dashboard-pill muted">v{overview?.version ?? 'n/a'}</span>
      {#if currentUser}
        <span class="dashboard-pill muted">Signed in as {currentUser}</span>
      {/if}
    </div>
  </header>

  {#if loading}
    <div class="dashboard-banner" data-testid="dashboard-loading">
      Loading dashboard data...
    </div>
  {/if}

  {#if error}
    <div class="dashboard-banner error" role="alert" data-testid="dashboard-error">
      {error}
    </div>
  {/if}

  <section class="metric-grid" aria-label="Server stats">
    <article class="metric-card" data-testid="dashboard-metric-load">
      <span>CPU</span>
      <strong>{formatCpuUsage(overview?.cpu_usage_percent)}</strong>
      <small>Live usage</small>
    </article>

    <article class="metric-card" data-testid="dashboard-metric-memory">
      <span>Memory used</span>
      <strong>
        {#if overview?.memory_used_bytes !== null &&
          overview?.memory_used_bytes !== undefined &&
          overview?.memory_total_bytes !== null &&
          overview?.memory_total_bytes !== undefined}
          {formatCapacity(overview.memory_used_bytes, overview.memory_total_bytes)}
        {:else}
          n/a
        {/if}
      </strong>
      <small>
        {#if overview?.memory_used_bytes !== null &&
          overview?.memory_used_bytes !== undefined &&
          overview?.memory_total_bytes !== null &&
          overview?.memory_total_bytes !== undefined}
          {formatPercent(overview.memory_used_bytes, overview.memory_total_bytes)}
          used
        {:else}
          n/a
        {/if}
      </small>
    </article>

    <article class="metric-card" data-testid="dashboard-metric-root-disk">
      <span>Root disk used</span>
      <strong>
        {formatPercent(
          overview?.disks.find((disk) => disk.mount_point === '/')?.used_bytes,
          overview?.disks.find((disk) => disk.mount_point === '/')?.total_bytes,
        )}
      </strong>
      <small>
        {formatCapacity(
          overview?.disks.find((disk) => disk.mount_point === '/')?.used_bytes,
          overview?.disks.find((disk) => disk.mount_point === '/')?.total_bytes,
        )}
      </small>
    </article>

    {#if overview?.disks.find((disk) => disk.mount_point === '/mnt/games')}
      <article class="metric-card" data-testid="dashboard-metric-games-disk">
        <span>Games disk used</span>
        <strong>
          {formatPercent(
            overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.used_bytes,
            overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.total_bytes,
          )}
        </strong>
        <small>
          {formatCapacity(
            overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.used_bytes,
            overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.total_bytes,
          )}
        </small>
      </article>
    {/if}

    <article class="metric-card" data-testid="dashboard-metric-terminals">
      <span>Active terminals</span>
      <strong>{overview?.terminal_count ?? 'n/a'}</strong>
      <small>Live PTY sessions</small>
    </article>

    <article class="metric-card warning" data-testid="dashboard-metric-failed-services">
      <span>Failed services</span>
      <strong>{overview?.service_summary.failed ?? 'n/a'}</strong>
      <small>systemd units reporting failure</small>
    </article>
  </section>

  <section class="dashboard-grid">
    <article class="dashboard-card identity-card" data-testid="dashboard-identity">
      <div class="card-head">
        <div>
          <p class="eyebrow">Identity</p>
          <h3>Server details</h3>
        </div>
      </div>

      <dl class="info-list">
        <div>
          <dt>Hostname</dt>
          <dd>{overview?.hostname ?? 'n/a'}</dd>
        </div>
        <div>
          <dt>Uptime</dt>
          <dd>{formatUptime(overview?.uptime_seconds)}</dd>
        </div>
        <div>
          <dt>IP addresses</dt>
          <dd>{formatList(overview?.primary_ips ?? [])}</dd>
        </div>
        <div>
          <dt>Current user</dt>
          <dd>{currentUser ?? 'n/a'}</dd>
        </div>
        <div>
          <dt>HomePanel version</dt>
          <dd>{overview?.version ?? 'n/a'}</dd>
        </div>
      </dl>
    </article>

    <article class="dashboard-card health-card" data-testid="dashboard-health">
      <div class="card-head">
        <div>
          <p class="eyebrow">Health</p>
          <h3>Service and storage health</h3>
        </div>
      </div>

      <div class="health-stack">
        <div class="health-row">
          <span>HomePanel API</span>
          <strong>{error ? 'Offline' : 'Online'}</strong>
        </div>
        <div class="health-row">
          <span>Storage path</span>
          <strong>{overview?.storage_path ?? 'n/a'}</strong>
        </div>
        <div class="health-row">
          <span>Database path</span>
          <strong>{overview?.database_path ?? 'n/a'}</strong>
        </div>
        <div class="health-row">
          <span>Services</span>
          <strong>
            {#if overview?.service_summary.total === null ||
              overview?.service_summary.total === undefined}
              n/a
            {:else}
              {overview?.service_summary.running ?? 0} running ·
              {overview?.service_summary.failed ?? 0} failed ·
              {overview?.service_summary.total} total
            {/if}
          </strong>
        </div>
      </div>
    </article>

    <article class="dashboard-card actions-card" data-testid="dashboard-actions">
      <div class="card-head">
        <div>
          <p class="eyebrow">Quick actions</p>
          <h3>Open common sections</h3>
        </div>
      </div>

      <div class="action-list">
        <button
          type="button"
          class="action-tile primary"
          data-testid="dashboard-action-terminals"
          on:click={() => onNavigate('terminals')}
        >
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="m6 8 4 4-4 4"/>
              <path d="M12 16h6"/>
            </svg>
          </span>
          <span class="action-copy">
            <strong>Terminal</strong>
            <small>Open shell sessions</small>
          </span>
        </button>
        <button
          type="button"
          class="action-tile primary"
          data-testid="dashboard-action-files"
          on:click={() => onNavigate('files')}
        >
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M3.5 7.5A2.5 2.5 0 0 1 6 5h4l1.5 2H18a2.5 2.5 0 0 1 2.5 2.5v7A2.5 2.5 0 0 1 18 19H6a2.5 2.5 0 0 1-2.5-2.5z"/>
            </svg>
          </span>
          <span class="action-copy">
            <strong>Files</strong>
            <small>Browse server storage</small>
          </span>
        </button>
        <button
          type="button"
          class="action-tile primary"
          data-testid="dashboard-action-services"
          on:click={() => onNavigate('services')}
        >
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M12 3.5 14 8l4.5.5-3.2 3.2.9 4.6L12 14.8 7.8 16.3l.9-4.6L5.5 8.5 10 8z"/>
            </svg>
          </span>
          <span class="action-copy">
            <strong>Services</strong>
            <small>Check systemd units</small>
          </span>
        </button>
        <button
          type="button"
          class="action-tile muted"
          data-testid="dashboard-action-logs"
          on:click={() => onNavigate('logs')}
        >
          <span class="action-icon" aria-hidden="true">
            <svg viewBox="0 0 24 24">
              <path d="M5 5.5h14v13H5z"/>
              <path d="M8 9h8"/>
              <path d="M8 12h8"/>
              <path d="M8 15h5"/>
            </svg>
          </span>
          <span class="action-copy">
            <strong>Logs</strong>
            <small>Review recent output</small>
          </span>
        </button>
      </div>
    </article>

    <article class="dashboard-card storage-card" data-testid="dashboard-storage">
      <div class="card-head">
        <div>
          <p class="eyebrow">Storage</p>
          <h3>Important mounts</h3>
        </div>
      </div>

      <div class="mount-list">
        <div class="mount-row" data-testid="dashboard-mount-root">
          <div>
            <span>/</span>
            <small>Root filesystem</small>
          </div>
          <strong>
            {formatCapacity(
              overview?.disks.find((disk) => disk.mount_point === '/')?.used_bytes,
              overview?.disks.find((disk) => disk.mount_point === '/')?.total_bytes,
            )}
          </strong>
        </div>

        {#if overview?.disks.find((disk) => disk.mount_point === '/mnt/games')}
          <div class="mount-row" data-testid="dashboard-mount-games">
            <div>
              <span>/mnt/games</span>
              <small>Game data</small>
            </div>
            <strong>
              {formatCapacity(
                overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.used_bytes,
                overview?.disks.find((disk) => disk.mount_point === '/mnt/games')?.total_bytes,
              )}
            </strong>
          </div>
        {/if}

        {#if overview?.disks.find((disk) => disk.mount_point === '/var/lib/homepanel') &&
          !sameDiskSignature(
            overview?.disks.find((disk) => disk.mount_point === '/var/lib/homepanel') ?? null,
            overview?.disks.find((disk) => disk.mount_point === '/') ?? null,
          )}
          <div class="mount-row" data-testid="dashboard-mount-homepanel">
            <div>
              <span>/var/lib/homepanel</span>
              <small>HomePanel data</small>
            </div>
            <strong>
              {formatCapacity(
                overview?.disks.find((disk) => disk.mount_point === '/var/lib/homepanel')?.used_bytes,
                overview?.disks.find((disk) => disk.mount_point === '/var/lib/homepanel')?.total_bytes,
              )}
            </strong>
          </div>
        {/if}
      </div>
    </article>
  </section>
</section>

<style>
  .dashboard-page {
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: auto;
    padding: 18px 22px 22px;
    display: grid;
    align-content: start;
    gap: 14px;
    background: transparent;
  }

  .dashboard-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 14px;
  }

  .dashboard-heading {
    min-width: 0;
    display: grid;
    gap: 4px;
  }

  .dashboard-heading h2 {
    margin: 0;
    font-size: 1.42rem;
    line-height: 1.15;
    letter-spacing: 0;
  }

  .dashboard-subtitle {
    margin: 0;
    color: var(--muted);
    font-size: 0.88rem;
    line-height: 1.45;
  }

  .dashboard-header-meta {
    display: flex;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .dashboard-pill {
    min-height: 26px;
    display: inline-flex;
    align-items: center;
    padding: 4px 9px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--surface-strong);
    color: var(--text);
    font-size: 0.76rem;
    font-weight: 700;
    white-space: nowrap;
  }

  .dashboard-pill.muted {
    color: var(--muted);
  }

  .dashboard-banner {
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface-strong);
    color: var(--muted);
    font-size: 0.86rem;
  }

  .dashboard-banner.error {
    border-color: #d8a6a6;
    background: var(--danger-soft);
    color: var(--danger);
  }

  .metric-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(170px, 1fr));
    gap: 10px;
  }

  .metric-card,
  .dashboard-card {
    min-width: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface-strong);
    box-shadow: 0 1px 0 rgba(32, 38, 45, 0.04), 0 6px 14px rgba(32, 38, 45, 0.04);
  }

  .metric-card {
    display: grid;
    gap: 4px;
    min-height: 94px;
    padding: 12px;
  }

  .metric-card span {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .metric-card strong {
    font-family: var(--mono);
    font-size: 1.08rem;
    font-weight: 700;
    letter-spacing: 0;
  }

  .metric-card small {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .metric-card.warning strong {
    color: var(--danger);
  }

  .dashboard-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .dashboard-card {
    padding: 14px;
  }

  .storage-card {
    grid-column: 1 / -1;
  }

  .card-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }

  .card-head h3 {
    margin: 0;
    font-size: 0.98rem;
    line-height: 1.2;
  }

  .info-list,
  .health-stack,
  .mount-list {
    display: grid;
    gap: 10px;
  }

  .info-list {
    margin: 0;
  }

  .info-list div {
    display: grid;
    gap: 3px;
  }

  .info-list dt,
  .health-row span {
    color: var(--muted);
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0;
    text-transform: uppercase;
  }

  .info-list dd,
  .health-row strong,
  .mount-row strong {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: 0.92rem;
  }

  .health-row {
    display: grid;
    gap: 4px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
  }

  .health-row:first-child {
    padding-top: 0;
    border-top: 0;
  }

  .action-list {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }

  .action-tile {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 60px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 13px;
    background: var(--surface-strong);
    box-shadow: 0 1px 0 rgba(32, 38, 45, 0.04);
    color: var(--text);
    text-align: left;
  }

  .action-tile:hover {
    border-color: var(--line-strong);
  }

  .action-tile.primary {
    background: #f8fafc;
  }

  .action-tile.muted {
    color: var(--muted);
  }

  .action-icon {
    width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--line);
    color: var(--muted);
  }

  .action-icon svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-linecap: round;
    stroke-linejoin: round;
    stroke-width: 1.8;
  }

  .action-copy {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .action-copy strong {
    font-size: 0.88rem;
    line-height: 1.2;
  }

  .action-copy small {
    color: var(--muted);
    font-size: 0.76rem;
    line-height: 1.2;
  }

  .mount-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 0;
    border-top: 1px solid var(--line);
  }

  .mount-row:first-child {
    padding-top: 0;
    border-top: 0;
  }

  .mount-row div {
    display: grid;
    gap: 3px;
  }

  .mount-row span {
    font-weight: 700;
  }

  .mount-row small {
    color: var(--muted);
    font-size: 0.8rem;
  }

  .mount-row strong {
    font-family: var(--mono);
    white-space: nowrap;
  }

  @media (max-width: 980px) {
    .dashboard-header {
      flex-direction: column;
    }

    .dashboard-grid {
      grid-template-columns: 1fr;
    }

    .storage-card {
      grid-column: auto;
    }

    .action-list {
      grid-template-columns: 1fr;
    }
  }
</style>
