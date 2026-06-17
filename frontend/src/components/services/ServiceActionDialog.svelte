<script lang="ts">
  export let open = false;
  export let serviceName = '';
  export let action: 'stop' | 'restart' = 'stop';
  export let warning = '';
  export let error = '';
  export let loading = false;
  export let onConfirm: () => void;
  export let onClose: () => void;

  $: actionLabel = action === 'restart' ? 'Restart' : 'Stop';
  $: loadingLabel = action === 'restart' ? 'Restarting...' : 'Stopping...';

  function handleBackdropClick() {
    if (!loading) {
      onClose();
    }
  }
</script>

{#if open}
  <button
    type="button"
    class="dialog-backdrop"
    aria-label="Close dialog"
    on:click={handleBackdropClick}
  ></button>
  <form
    class="dialog service-action-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="service-action-title"
    aria-describedby="service-action-warning"
    on:submit|preventDefault={onConfirm}
  >
    <div class="dialog-head">
      <div>
        <p class="eyebrow">Dangerous action</p>
        <h2 id="service-action-title">
          {actionLabel} {serviceName}?
        </h2>
        <p>Confirm before systemd sends the request to this service.</p>
      </div>
      <button
        type="button"
        class="icon-button"
        aria-label="Close dialog"
        disabled={loading}
        on:click={onClose}>x</button
      >
    </div>

    <div class="service-action-summary" data-testid="service-action-summary">
      <div>
        <span>Service</span>
        <strong>{serviceName}</strong>
      </div>
      <div>
        <span>Action</span>
        <strong>{actionLabel}</strong>
      </div>
    </div>

    <div
      class="service-action-warning"
      data-testid="service-action-warning"
      id="service-action-warning"
    >
      <p>{warning}</p>
    </div>

    {#if error}
      <div class="notice error" role="alert" data-testid="service-action-error">
        {error}
      </div>
    {/if}

    <div class="dialog-actions">
      <button type="button" on:click={onClose} disabled={loading}
        >Cancel</button
      >
      <button
        class="primary-button danger-button"
        type="submit"
        disabled={loading}
        data-testid="service-action-confirm"
      >
        {#if loading}
          {loadingLabel}
        {:else}
          Confirm
        {/if}
      </button>
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
    width: min(100% - 28px, 460px);
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

  .service-action-summary {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .service-action-summary div {
    display: grid;
    gap: 4px;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    background: var(--surface);
  }

  .service-action-summary span {
    color: var(--muted);
    font-size: 0.74rem;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .service-action-warning {
    padding: 12px;
    border: 1px solid #c08383;
    border-radius: var(--radius);
    background: var(--danger-soft);
    color: var(--danger);
  }

  .service-action-warning p {
    margin: 0;
    color: inherit;
    font-size: 0.88rem;
    line-height: 1.45;
  }

  .dialog-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  @media (max-width: 520px) {
    .service-action-summary,
    .dialog-actions {
      display: grid;
      grid-template-columns: 1fr;
    }
  }
</style>
