<script lang="ts">
  import { onMount } from 'svelte';
  import '@xterm/xterm/css/xterm.css';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';

  export let terminalId: string;
  export let clearToken = 0;
  export let terminalStatus: string | undefined = undefined;
  export let isKilling = false;
  export let onStatusChange: ((status: string) => void) | undefined = undefined;
  export let onAttachError: ((message: string) => void) | undefined = undefined;

  let host: HTMLDivElement;
  let instance: Terminal | null = null;
  let socket: WebSocket | null = null;
  let disposed = false;
  let reconnectTimer: number | undefined;
  let lastClearToken = clearToken;
  let resizeObserver: ResizeObserver | null = null;
  let fitAddon: FitAddon | null = null;
  let fitFrame: number | null = null;
  let fitDelay: number | undefined;
  let receivedHandshake = false;

  function decodeBase64(data: string) {
    const binary = atob(data);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i += 1) {
      bytes[i] = binary.charCodeAt(i);
    }
    return new TextDecoder().decode(bytes);
  }

  function sendInput(data: string) {
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(JSON.stringify({ type: 'input', data }));
    }
  }

  function warnClipboard(message: string) {
    console.warn(`[HomePanel] ${message}`);
  }

  function stripBracketedPasteWrappers(text: string) {
    return text.split('\u001b[200~').join('').split('\u001b[201~').join('');
  }

  async function writeClipboardText(text: string) {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(text);
        return true;
      }
    } catch (error) {
      console.warn(error);
    }

    try {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.setAttribute('readonly', 'true');
      textarea.style.position = 'fixed';
      textarea.style.left = '-9999px';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      const copied = document.execCommand('copy');
      textarea.remove();
      return copied;
    } catch (error) {
      console.warn(error);
      return false;
    }
  }

  async function readClipboardTextFallback() {
    try {
      if (navigator.clipboard?.readText) {
        return await navigator.clipboard.readText();
      }
    } catch (error) {
      console.warn(error);
    }
    return '';
  }

  function isCopyShortcut(event: KeyboardEvent) {
    return (event.ctrlKey || event.metaKey) && event.code === 'KeyC';
  }

  function isPasteShortcut(event: KeyboardEvent) {
    return (
      ((event.ctrlKey || event.metaKey) && event.code === 'KeyV') ||
      (event.shiftKey && event.code === 'Insert')
    );
  }

  function handleCopyEvent(event: ClipboardEvent) {
    if (!instance) return;
    const selection = instance.getSelection();
    if (!selection) return;

    const clipboardText = selection.trimEnd();
    if (event.clipboardData) {
      event.preventDefault();
      event.clipboardData.setData('text/plain', clipboardText);
    }
    instance.clearSelection();
    instance.focus();

    void writeClipboardText(clipboardText).catch((error) => {
      warnClipboard(
        'Clipboard copy failed. Browser clipboard access may require HTTPS or localhost.',
      );
      console.warn(error);
    });
  }

  async function handlePasteEvent(event: ClipboardEvent) {
    event.preventDefault();
    event.stopPropagation();

    const fromEvent = event.clipboardData?.getData('text/plain') ?? '';
    const directPaste = stripBracketedPasteWrappers(fromEvent);
    if (directPaste) {
      sendInput(directPaste);
      instance?.focus();
      return;
    }

    const fallback = stripBracketedPasteWrappers(await readClipboardTextFallback());
    if (fallback) {
      sendInput(fallback);
      instance?.focus();
      return;
    }

    warnClipboard(
      'Clipboard paste failed. Browser clipboard access may require HTTPS or localhost.',
    );
  }

  function connect() {
    if (disposed) return;
    receivedHandshake = false;
    socket = new WebSocket(
      `${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}/api/terminals/${terminalId}/ws`,
    );
    socket.onopen = () => {
      instance?.focus();
      if (instance) {
        socket?.send(
          JSON.stringify({
            type: 'resize',
            cols: instance.cols,
            rows: instance.rows,
          }),
        );
      }
    };
    socket.onmessage = (event) => {
      const message = JSON.parse(event.data);
      if (message.type === 'hello' || message.type === 'status') {
        receivedHandshake = true;
        if (message.status) {
          onStatusChange?.(String(message.status));
        }
      } else if (message.type === 'scrollback' || message.type === 'output') {
        instance?.write(decodeBase64(message.data));
      } else if (message.type === 'exit') {
        onStatusChange?.('exited');
        instance?.write(
          `\r\n[process exited: ${message.code ?? 'unknown'}]\r\n`,
        );
      }
    };
    socket.onerror = () => {
      if (disposed) return;
      if (!receivedHandshake && !isKilling) {
        onStatusChange?.('failed');
        onAttachError?.(
          'Terminal process is no longer available. Create a new terminal.',
        );
      }
    };
    socket.onclose = () => {
      if (disposed) return;
      if (isKilling) {
        onStatusChange?.('exited');
        return;
      }
      if (!receivedHandshake) {
        onStatusChange?.('failed');
        onAttachError?.(
          'Terminal process is no longer available. Create a new terminal.',
        );
        return;
      }
      const currentStatus = terminalStatus?.toLowerCase() ?? '';
      if (currentStatus === 'exited' || currentStatus === 'failed') {
        onStatusChange?.(currentStatus);
        return;
      }
      onStatusChange?.('detached');
      instance?.write('\r\n[disconnected]\r\n');
      reconnectTimer = window.setTimeout(connect, 1500);
    };
  }

  function syncClearToken(nextClearToken: number) {
    if (!instance || nextClearToken === lastClearToken) return;
    lastClearToken = nextClearToken;
    instance.clear();
    instance.focus();
  }

  function sendResize() {
    if (socket?.readyState === WebSocket.OPEN) {
      socket.send(
        JSON.stringify({
          type: 'resize',
          cols: instance?.cols ?? 120,
          rows: instance?.rows ?? 32,
        }),
      );
    }
  }

  function getFrameElement() {
    return host?.parentElement as HTMLElement | null;
  }

  function clearFrameSnap() {
    getFrameElement()?.style.removeProperty('height');
  }

  function fitTerminal() {
    if (!instance || !fitAddon || disposed) return;
    fitFrame = null;
    fitAddon.fit();
    sendResize();

    const frame = getFrameElement();
    const screen = host.querySelector('.xterm-screen') as HTMLElement | null;
    const screenHeight = screen?.getBoundingClientRect().height ?? 0;
    if (frame && screenHeight > 0) {
      const frameStyle = getComputedStyle(frame);
      const borderTop = Number.parseFloat(frameStyle.borderTopWidth) || 0;
      const borderBottom = Number.parseFloat(frameStyle.borderBottomWidth) || 0;
      frame.style.height = `${Math.round(screenHeight + borderTop + borderBottom)}px`;
    }
  }

  function scheduleFit() {
    if (disposed) return;
    clearFrameSnap();
    if (fitFrame !== null) cancelAnimationFrame(fitFrame);
    if (fitDelay) window.clearTimeout(fitDelay);
    fitFrame = requestAnimationFrame(() => {
      fitTerminal();
      fitDelay = window.setTimeout(fitTerminal, 50);
    });
  }

  onMount(() => {
    fitAddon = new FitAddon();
    instance = new Terminal({
      fontFamily:
        'ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace',
      fontSize: 13,
      cursorBlink: true,
      allowTransparency: true,
      theme: {
        background: '#0c1117',
        foreground: '#e7edf4',
        cursor: '#9cc0ff',
        selectionBackground: 'rgba(90, 141, 255, 0.3)',
      },
    });
    instance.loadAddon(fitAddon);
    instance.open(host);
    instance.attachCustomKeyEventHandler((event) => {
      if (event.type !== 'keydown') return true;
      const term = instance;
      if (!term) return true;

      if (isCopyShortcut(event)) {
        return !term.getSelection();
      }

      if (isPasteShortcut(event)) {
        return false;
      }

      return true;
    });
    host.addEventListener('copy', handleCopyEvent, true);
    host.addEventListener('paste', handlePasteEvent, true);
    scheduleFit();
    connect();
    instance.onData(
      (data) => sendInput(data),
    );
    resizeObserver = new ResizeObserver(() => scheduleFit());
    resizeObserver.observe(host);
    window.addEventListener('resize', scheduleFit);
    scheduleFit();
    return () => {
      disposed = true;
      if (reconnectTimer) window.clearTimeout(reconnectTimer);
      if (fitFrame !== null) cancelAnimationFrame(fitFrame);
      if (fitDelay) window.clearTimeout(fitDelay);
      clearFrameSnap();
      resizeObserver?.disconnect();
      window.removeEventListener('resize', scheduleFit);
      host?.removeEventListener('copy', handleCopyEvent, true);
      host?.removeEventListener('paste', handlePasteEvent, true);
      socket?.close();
      instance?.dispose();
      fitAddon = null;
    };
  });

  $: syncClearToken(clearToken);
</script>

<div class="terminal" data-testid="xterm-host" bind:this={host}></div>

<style>
  .terminal {
    width: 100%;
    height: 100%;
    min-height: 0;
    display: block;
    overflow: hidden;
    background: #0b1015;
  }
  :global(.xterm) {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 0;
  }
  :global(.xterm-viewport) {
    width: 100%;
  }
  :global(.xterm-screen) {
    display: block;
    width: 100%;
    min-height: 0;
  }
</style>
