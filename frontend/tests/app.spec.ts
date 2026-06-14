import { expect, type APIRequestContext, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';

type TerminalFixture = {
  id: string;
  name: string;
  kind: 'shell';
  status: string;
  command: string;
  cwd: string;
  cols: number;
  rows: number;
  exit_code: number | null;
  last_attached_at: string | null;
};

async function requireBackend(request: APIRequestContext) {
  let response;
  try {
    response = await request.get('/api/health', { timeout: 3_000 });
  } catch (error) {
    throw new Error(
      `HomePanel backend is not reachable through the Vite proxy at /api/health. Start homepaneld before running Playwright tests. Original error: ${String(
        error,
      )}`,
      { cause: error },
    );
  }

  if (!response.ok()) {
    throw new Error(
      `HomePanel backend health check failed: ${response.status()} ${await response.text()}`,
    );
  }
}

async function installMockWebSocket(
  page: Parameters<typeof test>[0]['page'],
  options?: {
    scrollback?: string;
  },
) {
  await page.addInitScript((mockOptions) => {
    const openedWebSockets: string[] = [];
    const sentWebSocketMessages: string[] = [];

    Object.defineProperty(window, '__homepanelWebSockets', {
      configurable: true,
      get() {
        return openedWebSockets;
      },
    });

    Object.defineProperty(window, '__homepanelWebSocketMessages', {
      configurable: true,
      get() {
        return sentWebSocketMessages;
      },
    });

    class MockWebSocket {
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSING = 2;
      static CLOSED = 3;

      url: string;
      readyState = 0;
      onopen: ((event: Event) => void) | null = null;
      onmessage: ((event: MessageEvent<string>) => void) | null = null;
      onclose: ((event: CloseEvent) => void) | null = null;
      onerror: ((event: Event) => void) | null = null;

      constructor(url: string) {
        this.url = url;
        openedWebSockets.push(url);
        const terminalId = new URL(url).pathname.split('/').at(-2) ?? '';
        const scrollback =
          (window as typeof window & { __homepanelScrollback?: string })
            .__homepanelScrollback ?? mockOptions.scrollback ?? '/bin/bash\r\nhomepanel$ ';
        queueMicrotask(() => {
          this.readyState = 1;
          this.onopen?.(new Event('open'));
          queueMicrotask(() => {
            this.onmessage?.(
              new MessageEvent('message', {
                data: JSON.stringify({
                  type: 'hello',
                  terminal_id: terminalId,
                  status: 'running',
                }),
              }),
            );
            this.onmessage?.(
              new MessageEvent('message', {
                data: JSON.stringify({
                  type: 'scrollback',
                  data: btoa(scrollback),
                }),
              }),
            );
          });
        });
      }

      send(data: string) {
        sentWebSocketMessages.push(String(data));
      }

      close() {
        this.readyState = 3;
        this.onclose?.(new CloseEvent('close'));
      }

      addEventListener() {}

      removeEventListener() {}

      dispatchEvent() {
        return true;
      }
    }

    // @ts-expect-error test-only websocket shim
    window.WebSocket = MockWebSocket;
  }, { scrollback: options?.scrollback ?? '/bin/bash\r\nhomepanel$ ' });
}

async function installMockClipboard(
  page: Parameters<typeof test>[0]['page'],
  options?: {
    readText?: string;
    failRead?: boolean;
    failWrite?: boolean;
  },
) {
  await page.addInitScript((mockOptions) => {
    const clipboardWrites: string[] = [];
    Object.defineProperty(window, '__homepanelClipboardWrites', {
      configurable: true,
      get() {
        return clipboardWrites;
      },
    });
    const clipboard = {
      async readText() {
        if (mockOptions.failRead) {
          throw new Error('clipboard read blocked');
        }
        return mockOptions.readText ?? '';
      },
      async writeText(text: string) {
        if (mockOptions.failWrite) {
          throw new Error('clipboard write blocked');
        }
        clipboardWrites.push(text);
      },
    };

    Object.defineProperty(navigator, 'clipboard', {
      configurable: true,
      value: clipboard,
    });
  }, {
    readText: options?.readText ?? '',
    failRead: options?.failRead ?? false,
    failWrite: options?.failWrite ?? false,
  });
}

function makeTerminal(
  overrides: Partial<TerminalFixture> & Pick<TerminalFixture, 'id' | 'name'>,
): TerminalFixture {
  return {
    kind: 'shell',
    status: 'running',
    command: '/bin/bash',
    cwd: '/home',
    cols: 120,
    rows: 32,
    exit_code: null,
    last_attached_at: null,
    ...overrides,
  };
}

async function mockAuthenticatedShell(
  page: Parameters<typeof test>[0]['page'],
  terminals: TerminalFixture[],
  options?: {
    onKill?: (terminalId: string) => Promise<void> | void;
    scrollback?: string;
    clipboard?: {
      readText?: string;
      failRead?: boolean;
      failWrite?: boolean;
    };
  },
) {
  await installMockWebSocket(page, { scrollback: options?.scrollback });
  await installMockClipboard(page, options?.clipboard);

  let state = terminals.slice();

  await page.route('/api/auth/me', async (route) => {
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({ username: 'admin' }),
    });
  });

  await page.route('**/api/terminals**', async (route) => {
    const url = new URL(route.request().url());
    if (route.request().method() === 'GET' && url.pathname === '/api/terminals') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify(state),
      });
      return;
    }

    if (
      route.request().method() === 'POST' &&
      url.pathname.endsWith('/kill')
    ) {
      const terminalId = url.pathname.split('/')[3];
      await options?.onKill?.(terminalId);
      state = state.filter((terminal) => terminal.id !== terminalId);
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ ok: true }),
      });
      return;
    }

    await route.fallback();
  });

  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Terminals' })).toBeVisible();
}

async function dispatchPasteText(
  page: Parameters<typeof test>[0]['page'],
  text: string,
) {
  await page.evaluate((payload) => {
    const host = document.querySelector<HTMLElement>('[data-testid="xterm-host"]');
    if (!host) {
      throw new Error('xterm host is missing');
    }

    const dataTransfer = new DataTransfer();
    dataTransfer.setData('text/plain', payload);

    const event = new ClipboardEvent('paste', {
      bubbles: true,
      cancelable: true,
      clipboardData: dataTransfer,
    });

    host.dispatchEvent(event);
  }, text);
}

async function dispatchClipboardEvent(
  page: Parameters<typeof test>[0]['page'],
  type: 'copy' | 'cut' | 'paste',
  text: string,
) {
  await page.evaluate(
    ({ eventType, payload }) => {
      const host = document.querySelector<HTMLElement>('[data-testid="xterm-host"]');
      if (!host) {
        throw new Error('xterm host is missing');
      }

      const dataTransfer = new DataTransfer();
      dataTransfer.setData('text/plain', payload);

      const event = new ClipboardEvent(eventType, {
        bubbles: true,
        cancelable: true,
        clipboardData: dataTransfer,
      });

      host.dispatchEvent(event);
    },
    { eventType: type, payload: text },
  );
}

async function dispatchShortcutKey(
  page: Parameters<typeof test>[0]['page'],
  options: {
    key: string;
    code: string;
    ctrlKey?: boolean;
    metaKey?: boolean;
    shiftKey?: boolean;
  },
) {
  await page.evaluate((shortcut) => {
    const host = document.querySelector<HTMLElement>('[data-testid="xterm-host"]');
    if (!host) {
      throw new Error('xterm host is missing');
    }

    const event = new KeyboardEvent('keydown', {
      bubbles: true,
      cancelable: true,
      key: shortcut.key,
      code: shortcut.code,
      ctrlKey: shortcut.ctrlKey ?? false,
      metaKey: shortcut.metaKey ?? false,
      shiftKey: shortcut.shiftKey ?? false,
    });

    host.dispatchEvent(event);
  }, options);
}

test('opens the HomePanel app', async ({ page, request }) => {
  await requireBackend(request);

  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'HomePanel' })).toBeVisible();
});

test('authenticated app shell visual smoke @visual', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [makeTerminal({ id: 'visual-terminal-0001', name: 'System shell', status: 'starting' })]);

  const sessionList = page.getByTestId('session-list');
  await expect(
    sessionList.getByTestId('session-tab-visual-terminal-0001'),
  ).toBeVisible();

  const activeTerminal = page.getByTestId('active-terminal-panel');
  await expect(
    activeTerminal.getByTestId('active-terminal-command'),
  ).toHaveText('/bin/bash');
  await expect(activeTerminal.getByTestId('active-terminal-cwd')).toHaveText(
    '/home',
  );
  await expect(activeTerminal.getByTestId('active-terminal-status')).toHaveText(
    /running/i,
  );
  await expect(
    sessionList.getByTestId('session-select-visual-terminal-0001'),
  ).toBeVisible();
  await expect(
    sessionList.getByTestId('session-close-visual-terminal-0001'),
  ).toBeVisible();
  await expect(activeTerminal.getByTestId('terminal-viewport')).toBeVisible();

  const terminalHost = page.getByTestId('xterm-host');
  const terminalScreen = page.locator('.xterm-screen');
  const hostBox = await terminalHost.boundingBox();
  const screenBox = await terminalScreen.boundingBox();
  expect(hostBox).not.toBeNull();
  expect(screenBox).not.toBeNull();
  if (hostBox && screenBox) {
    expect(Math.abs(hostBox.height - screenBox.height)).toBeLessThan(6);
  }

  const accessibility = await new AxeBuilder({ page })
    .include('.app-shell')
    .analyze();
  expect(accessibility.violations).toEqual([]);
});

test('clipboard shortcuts copy selection and paste clipboard text', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(
    page,
    [makeTerminal({ id: 'terminal-clip', name: 'Clipboard Shell' })],
    { scrollback: 'copy-test\r\n', clipboard: { readText: 'paste-test' } },
  );

  const terminalScreen = page.locator('.xterm-screen');
  const screenBox = await terminalScreen.boundingBox();
  expect(screenBox).not.toBeNull();
  if (!screenBox) {
    return;
  }

  await page.mouse.move(screenBox.x + 3, screenBox.y + 14);
  await page.mouse.down();
  await page.mouse.move(screenBox.x + 96, screenBox.y + 14);
  await page.mouse.up();
  await page.keyboard.press('Control+C');

  await expect(page.evaluate(() => window.__homepanelClipboardWrites)).resolves.toEqual(
    expect.arrayContaining(['copy-test']),
  );
  await expect(
    page.evaluate(() =>
      (window as typeof window & { __homepanelWebSocketMessages: string[] })
        .__homepanelWebSocketMessages.some((message) =>
          message.includes('\\u0003'),
        ),
    ),
  ).resolves.toBe(false);
  await page.keyboard.press('Control+V');
  await page.keyboard.press('Control+Shift+V');
  await page.keyboard.press('Shift+Insert');
  await expect(
    page.evaluate(
      () =>
        (window as typeof window & { __homepanelWebSocketMessages: string[] })
          .__homepanelWebSocketMessages.some((message) =>
            message.includes('\\u0016') ||
            message.includes('\\u001b[200~') ||
            message.includes('\\u001b[201~'),
          ),
    ),
  ).resolves.toBe(false);

  await dispatchPasteText(
    page,
    '\u001b[200~echo paste-test\u001b[201~',
  );
  await expect(page.evaluate(() => window.__homepanelWebSocketMessages)).resolves.toEqual(
    expect.arrayContaining([
      expect.stringContaining('"type":"input"'),
      expect.stringContaining('echo paste-test'),
    ]),
  );
  await expect(
    page.evaluate(() =>
      (window as typeof window & { __homepanelWebSocketMessages: string[] })
        .__homepanelWebSocketMessages.some((message) =>
          message.includes('\\u001b[200~') || message.includes('\\u001b[201~'),
        ),
    ),
  ).resolves.toBe(false);

  await page.mouse.click(screenBox.x + 12, screenBox.y + 14);
  await page.keyboard.press('Control+C');
  await expect(
    page.evaluate(() =>
      (window as typeof window & { __homepanelWebSocketMessages: string[] })
        .__homepanelWebSocketMessages.some((message) =>
          message.includes('\\u0003'),
        ),
    ),
  ).resolves.toBe(true);
});

test('clipboard shortcuts use physical key codes on non-English layouts', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(
    page,
    [makeTerminal({ id: 'terminal-layout', name: 'Layout Shell' })],
    { scrollback: 'layout-copy\r\n', clipboard: { readText: 'layout-paste' } },
  );

  const terminalScreen = page.locator('.xterm-screen');
  const screenBox = await terminalScreen.boundingBox();
  expect(screenBox).not.toBeNull();
  if (!screenBox) {
    return;
  }

  await page.mouse.move(screenBox.x + 3, screenBox.y + 14);
  await page.mouse.down();
  await page.mouse.move(screenBox.x + 120, screenBox.y + 14);
  await page.mouse.up();

  await dispatchShortcutKey(page, {
    key: 'с',
    code: 'KeyC',
    ctrlKey: true,
  });
  await dispatchClipboardEvent(page, 'copy', 'layout-copy');

  await expect(page.evaluate(() => window.__homepanelClipboardWrites)).resolves.toEqual(
    expect.arrayContaining(['layout-copy']),
  );
  await expect(
    page.evaluate(() =>
      (window as typeof window & { __homepanelWebSocketMessages: string[] })
        .__homepanelWebSocketMessages.some((message) =>
          message.includes('\\u0003'),
        ),
    ),
  ).resolves.toBe(false);

  await dispatchShortcutKey(page, {
    key: 'м',
    code: 'KeyV',
    ctrlKey: true,
  });
  await dispatchPasteText(page, '\u001b[200~layout-paste\u001b[201~');

  await expect(page.evaluate(() => window.__homepanelWebSocketMessages)).resolves.toEqual(
    expect.arrayContaining([
      expect.stringContaining('"type":"input"'),
      expect.stringContaining('layout-paste'),
    ]),
  );
  await expect(
    page.evaluate(() =>
      (window as typeof window & { __homepanelWebSocketMessages: string[] })
        .__homepanelWebSocketMessages.some((message) =>
          message.includes('\\u0016') ||
          message.includes('\\u001b[200~') ||
          message.includes('\\u001b[201~'),
        ),
    ),
  ).resolves.toBe(false);
});

test('clipboard failures do not crash the terminal', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(
    page,
    [makeTerminal({ id: 'terminal-clipboard-fail', name: 'Clipboard Fail Shell' })],
    {
      clipboard: {
        failRead: true,
        failWrite: true,
      },
    },
  );

  await dispatchPasteText(page, 'fallback-paste');
  await page.keyboard.press('Control+V');
  await page.keyboard.press('Control+C');

  await expect(page.getByRole('heading', { name: 'Clipboard Fail Shell' })).toBeVisible();
  await expect(page.evaluate(() => window.__homepanelWebSocketMessages)).resolves.toEqual(
    expect.arrayContaining([expect.stringContaining('fallback-paste')]),
  );
});

test('killing one of multiple terminals selects another live session', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  let resolveKill: (() => void) | null = null;
  const killBarrier = new Promise<void>((resolve) => {
    resolveKill = resolve;
  });
  await mockAuthenticatedShell(page, [
    makeTerminal({ id: 'terminal-a', name: 'Shell A' }),
    makeTerminal({ id: 'terminal-b', name: 'Shell B' }),
  ], {
    onKill: async (terminalId) => {
      if (terminalId === 'terminal-b') {
        await killBarrier;
      }
    },
  });

  const sessionList = page.getByTestId('session-list');
  await expect(sessionList.getByTestId('session-tab-terminal-a')).toBeVisible();
  await expect(sessionList.getByTestId('session-tab-terminal-b')).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Shell A' })).toBeVisible();
  await expect(page.evaluate(() => window.__homepanelWebSockets)).resolves.toEqual(
    expect.arrayContaining([expect.stringContaining('/api/terminals/terminal-a/ws')]),
  );

  await page.getByTestId('session-select-terminal-b').click();
  await expect(page.getByRole('heading', { name: 'Shell B' })).toBeVisible();
  await expect(page.evaluate(() => window.__homepanelWebSockets)).resolves.toEqual(
    expect.arrayContaining([expect.stringContaining('/api/terminals/terminal-b/ws')]),
  );
  await page.getByTestId('session-select-terminal-a').click();
  await expect(page.getByRole('heading', { name: 'Shell A' })).toBeVisible();
  await page.getByTestId('session-close-terminal-b').click();

  await expect(page.getByRole('heading', { name: 'Shell A' })).toBeVisible();
  await expect(
    page.getByTestId('session-tab-terminal-b'),
  ).toBeVisible();

  resolveKill?.();

  await expect(sessionList.getByTestId('session-tab-terminal-b')).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Shell B' })).toHaveCount(0);
  await expect(page.getByTestId('active-terminal-status')).toHaveText(
    /running/i,
  );
  await expect(page.getByTestId('terminal-viewport')).toBeVisible();
  await expect(page.evaluate(() => window.__homepanelWebSockets)).resolves.toEqual(
    expect.arrayContaining([expect.stringContaining('/api/terminals/terminal-b/ws')]),
  );
  await expect(page.getByTestId('xterm-host')).not.toHaveText('');
  await expect(page.getByTestId('terminal-viewport')).toBeVisible();
});

test('killing the last terminal shows an empty state', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [
    makeTerminal({ id: 'terminal-only', name: 'Solo Shell' }),
  ]);

  await expect(page.getByRole('heading', { name: 'Solo Shell' })).toBeVisible();
  await page.getByTestId('session-close-terminal-only').click();

  await expect(page.getByTestId('terminal-viewport')).toHaveCount(0);
  await expect(
    page.getByText('Start a shell on this host.'),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'New /bin/bash terminal' }),
  ).toBeVisible();
});
