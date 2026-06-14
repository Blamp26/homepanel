<script lang="ts">
  import type { TerminalSummary } from '../../api/terminals';

  export let terminals: TerminalSummary[] = [];
  export let activeId: string | null = null;
  export let onSelect: (id: string) => void;
  export let onClose: (id: string) => void;

  function shortId(id: string) {
    return id.slice(0, 8);
  }

  function statusClass(status: string) {
    const value = status.toLowerCase();
    if (value.includes('run')) return 'good';
    if (
      value.includes('exit') ||
      value.includes('dead') ||
      value.includes('stop')
    )
      return 'bad';
    return 'neutral';
  }

  function isSelectable(status: string) {
    const value = status.toLowerCase();
    return value !== 'exited' && value !== 'failed';
  }
</script>

{#if terminals.length > 0}
  <div
    class="session-list"
    aria-label="Terminal sessions"
    data-testid="session-list"
  >
    {#each terminals as terminal (terminal.id)}
      <div
        class:active={terminal.id === activeId}
        class="session-tab"
        role="presentation"
        data-testid={`session-tab-${terminal.id}`}
      >
        <button
          type="button"
          class="session-select"
          class:active={terminal.id === activeId}
          on:click={() => onSelect(terminal.id)}
          disabled={!isSelectable(terminal.status)}
          aria-label={`Select terminal ${terminal.name}`}
          data-testid={`session-select-${terminal.id}`}
        >
          <span class={`status-dot ${statusClass(terminal.status)}`}></span>
          <span class="session-copy">
            <strong>{terminal.name}</strong>
            <span>{terminal.command}</span>
            <small>{terminal.cwd} · {shortId(terminal.id)}</small>
          </span>
        </button>

        <button
          type="button"
          class="session-close"
          aria-label={`Close terminal ${terminal.name}`}
          on:click|preventDefault|stopPropagation={() => onClose(terminal.id)}
          on:mousedown|preventDefault|stopPropagation
          data-testid={`session-close-${terminal.id}`}
        >
          ×
        </button>
      </div>
    {/each}
  </div>
{:else}
  <div class="empty-sessions">
    <strong>No terminals yet</strong>
    <span>Create a shell to begin working on this host.</span>
  </div>
{/if}

<style>
  .session-list {
    min-height: 0;
    overflow: auto;
    display: grid;
    align-content: start;
    gap: 6px;
    padding-right: 2px;
  }

  .session-tab {
    width: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: stretch;
    gap: 6px;
    padding: 1px 0;
  }

  .session-select {
    min-width: 0;
    min-height: 68px;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 10px;
    align-items: center;
    padding: 10px 12px 10px 10px;
    border: 1px solid transparent;
    border-left: 3px solid transparent;
    border-radius: 0 var(--radius) var(--radius) 0;
    background: transparent;
    color: var(--text);
    text-align: left;
  }

  .session-tab:hover .session-select,
  .session-select.active {
    background: var(--surface-strong);
    border-color: var(--line);
    border-left-color: var(--accent);
  }

  .session-select:disabled {
    opacity: 0.7;
  }

  .session-select.active .session-copy span,
  .session-select.active .session-copy small {
    color: #475665;
  }

  .status-dot {
    margin-top: 4px;
  }

  .session-copy {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .session-copy strong,
  .session-copy span,
  .session-copy small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-copy strong {
    font-size: 0.9rem;
    line-height: 1.25;
  }

  .session-copy span,
  .session-copy small {
    color: var(--muted);
    font-family: var(--mono);
    font-size: 0.75rem;
  }

  .session-close {
    width: 32px;
    min-height: 68px;
    display: inline-grid;
    place-items: center;
    align-self: stretch;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: transparent;
    color: var(--muted);
    font-size: 1.1rem;
    line-height: 1;
    padding: 0;
  }

  .session-close:hover {
    border-color: var(--line);
    background: var(--surface-strong);
    color: var(--danger);
  }

  .empty-sessions {
    display: grid;
    gap: 4px;
    padding: 14px;
    border: 1px dashed var(--line-strong);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--muted);
  }

  .empty-sessions strong {
    color: var(--text);
  }
</style>
