<script lang="ts">
  export let open = false;
  export let onCreate: (payload: Record<string, unknown>) => void;
  export let onClose: () => void;

  let name = 'System shell';
  let command = '/bin/bash';
  let cwd = '/home';
  let kind = 'shell';

  const handleCreate = () => {
    onCreate({
      name,
      command,
      cwd,
      kind,
      cols: 120,
      rows: 32,
      env: {},
    });
  };
</script>

{#if open}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close dialog"
    on:click={onClose}
  ></button>
  <form
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="new-terminal-title"
    on:submit|preventDefault={handleCreate}
  >
    <div class="dialog-head">
      <div>
        <h2 id="new-terminal-title">New terminal</h2>
        <p>
          Start a persistent process and attach it to this browser workspace.
        </p>
      </div>
      <button
        type="button"
        class="icon-button"
        aria-label="Close dialog"
        on:click={onClose}>x</button
      >
    </div>

    <label for="terminal-name">Name</label>
    <input id="terminal-name" bind:value={name} required />

    <label for="terminal-command">Command</label>
    <input id="terminal-command" bind:value={command} required />

    <label for="terminal-cwd">Working directory</label>
    <input id="terminal-cwd" bind:value={cwd} required />

    <label for="terminal-kind">Kind</label>
    <select id="terminal-kind" bind:value={kind}>
      <option value="shell">Shell</option>
      <option value="command">Command</option>
      <option value="game_server">Game server</option>
      <option value="log_viewer">Log viewer</option>
    </select>

    <div class="dialog-actions">
      <button type="button" on:click={onClose}>Cancel</button>
      <button class="primary-button" type="submit">Create terminal</button>
    </div>
  </form>
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
    width: min(100% - 28px, 430px);
    transform: translate(-50%, -50%);
    display: grid;
    gap: 10px;
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

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  @media (max-width: 520px) {
    .dialog-actions {
      display: grid;
      grid-template-columns: 1fr;
    }
  }
</style>
