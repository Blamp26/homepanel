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

async function installMockWebSocket(page: Parameters<typeof test>[0]['page']) {
  await page.addInitScript(() => {
    const openedWebSockets: string[] = [];

    Object.defineProperty(window, '__homepanelWebSockets', {
      configurable: true,
      get() {
        return openedWebSockets;
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
                  data: btoa('/bin/bash\r\nhomepanel$ '),
                }),
              }),
            );
          });
        });
      }

      send() {}

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
  },
) {
  await installMockWebSocket(page);

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
