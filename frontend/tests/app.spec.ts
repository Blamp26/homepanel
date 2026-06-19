import { expect, type APIRequestContext, test } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { Buffer } from 'node:buffer';
import path from 'node:path';

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

type AuthFixture = {
  setupRequired: boolean;
  authenticated: boolean;
  username: string | null;
};

type ServiceFixture = {
  name: string;
  load: string;
  active: string;
  sub: string;
  description: string;
  unit_file_state?: string | null;
  details: {
    name: string;
    load_state: string;
    active_state: string;
    sub_state: string;
    unit_file_state: string;
    description: string;
    fragment_path: string | null;
    main_pid: number;
    memory_current: number | null;
    cpu_usage_nsec: number | null;
  };
  logs: string[];
};

type FileFixture = {
  path: string;
  kind: 'file' | 'dir' | 'symlink' | 'other';
  size?: number;
  modified?: string | null;
  readonly?: boolean;
  content?: string;
  allowedRoots?: string[];
};

type OverviewFixture = {
  api_status?: string;
  hostname?: string | null;
  uptime_seconds?: number | null;
  load_average?: [number, number, number] | null;
  cpu_usage_percent?: number | null;
  memory_total_bytes?: number | null;
  memory_used_bytes?: number | null;
  disks?: Array<{
    mount_point: string;
    total_bytes?: number | null;
    used_bytes?: number | null;
    available_bytes?: number | null;
  }>;
  primary_ips?: string[];
  terminal_count?: number;
  service_summary?: {
    total?: number | null;
    running?: number | null;
    failed?: number | null;
  };
  storage_path?: string;
  database_path?: string | null;
  version?: string;
};

type FilesFixtureOptions = {
  roots?: string[];
  visibleRoots?: string[];
  entries?: FileFixture[];
  failList?: boolean;
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

function makeFileFixture(overrides: Partial<FileFixture> & Pick<FileFixture, 'path' | 'kind'>): FileFixture {
  return {
    size: 0,
    modified: '2026-06-17T00:00:00.000Z',
    readonly: false,
    content: '',
    ...overrides,
  };
}

function normalizePath(input: string) {
  return input.replace(/\/+/g, '/');
}

function dirname(pathname: string) {
  const parent = path.posix.dirname(normalizePath(pathname));
  return parent === '.' ? '/' : parent;
}

function basename(pathname: string) {
  return path.posix.basename(normalizePath(pathname));
}

function sortFiles(entries: FileFixture[]) {
  return entries.slice().sort((a, b) => {
    const rank = (kind: FileFixture['kind']) => {
      switch (kind) {
        case 'dir':
          return 0;
        case 'symlink':
          return 1;
        case 'file':
          return 2;
        default:
          return 3;
      }
    };
    return rank(a.kind) - rank(b.kind) || basename(a.path).localeCompare(basename(b.path));
  });
}

function updateDescendantPaths(state: FileFixture[], source: string, target: string) {
  const normalizedSource = normalizePath(source);
  const normalizedTarget = normalizePath(target);
  return state.map((entry) => {
    if (entry.path === normalizedSource) {
      return { ...entry, path: normalizedTarget };
    }
    if (entry.path.startsWith(`${normalizedSource}/`)) {
      return {
        ...entry,
        path: normalizedTarget + entry.path.slice(normalizedSource.length),
      };
    }
    return entry;
  });
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
    auth?: AuthFixture;
    services?: ServiceFixture[];
    files?: FilesFixtureOptions;
    overview?: OverviewFixture;
    overviewSequence?: OverviewFixture[];
    overviewFailure?: boolean;
  },
) {
  await installMockWebSocket(page, { scrollback: options?.scrollback });
  await installMockClipboard(page, options?.clipboard);

  const authState: AuthFixture =
    options?.auth ?? {
      setupRequired: false,
      authenticated: true,
      username: 'alice',
    };

  let state = terminals.slice();
  const serviceState = options?.services ?? [];
  const serviceActions: string[] = [];
  const fileRoots = options?.files?.roots ?? ['/home', '/srv', '/DATA'];
  const visibleRoots =
    options?.files?.visibleRoots ?? fileRoots.filter((root) => root !== '/DATA');
  let fileState = (options?.files?.entries ?? [
    makeFileFixture({
      path: '/mnt',
      kind: 'dir',
    }),
    makeFileFixture({
      path: '/mnt/games',
      kind: 'dir',
    }),
    makeFileFixture({
      path: '/mnt/games/launcher.log',
      kind: 'file',
      size: 18,
      content: 'launcher started\n',
    }),
    makeFileFixture({
      path: '/srv',
      kind: 'dir',
    }),
    makeFileFixture({
      path: '/srv/app',
      kind: 'dir',
    }),
    makeFileFixture({
      path: '/srv/app/config.txt',
      kind: 'file',
      size: 18,
      content: 'name=homepanel\n',
    }),
    makeFileFixture({
      path: '/srv/app/readme.txt',
      kind: 'file',
      size: 26,
      content: 'Explorer test preview\n',
    }),
    makeFileFixture({
      path: '/srv/logs',
      kind: 'dir',
    }),
    makeFileFixture({
      path: '/srv/logs/daemon.log',
      kind: 'file',
      size: 19,
      content: 'daemon started\n',
    }),
  ]).map((entry) => ({
    ...entry,
    path: normalizePath(entry.path),
  }));
  const filesFailure = options?.files?.failList ?? false;
  const overviewState: OverviewFixture = {
    api_status: 'online',
    hostname: 'homepanel',
    uptime_seconds: 93_600,
    load_average: [0.12, 0.08, 0.05],
    cpu_usage_percent: 12,
    memory_total_bytes: 8 * 1024 ** 3,
    memory_used_bytes: 4 * 1024 ** 3,
    disks: [
      {
        mount_point: '/',
        total_bytes: 128 * 1024 ** 3,
        used_bytes: 64 * 1024 ** 3,
        available_bytes: 64 * 1024 ** 3,
      },
      ...(fileState.some((entry) => entry.path === '/mnt/games')
        ? [
            {
              mount_point: '/mnt/games',
              total_bytes: 512 * 1024 ** 3,
              used_bytes: 128 * 1024 ** 3,
              available_bytes: 384 * 1024 ** 3,
            },
          ]
        : []),
      ...(fileState.some((entry) => entry.path === '/var/lib/homepanel')
        ? [
            {
              mount_point: '/var/lib/homepanel',
              total_bytes: 64 * 1024 ** 3,
              used_bytes: 12 * 1024 ** 3,
              available_bytes: 52 * 1024 ** 3,
            },
          ]
        : []),
    ],
    primary_ips: ['192.168.1.10', '10.0.0.5'],
    terminal_count: state.length,
    service_summary: {
      total: serviceState.length,
      running: serviceState.filter(
        ({ active, sub }) => active === 'active' || sub === 'running',
      ).length,
      failed: serviceState.filter(
        ({ active, sub }) => active === 'failed' || sub === 'failed',
      ).length,
    },
    storage_path: '/var/lib/homepanel',
    database_path: '/var/lib/homepanel/homepanel.db',
    version: '0.1.0',
    ...(options?.overview ?? {}),
  };
  const overviewStates = (options?.overviewSequence?.length
    ? options.overviewSequence
    : [options?.overview ?? {}]
  ).map((overrides) => ({
    ...overviewState,
    ...overrides,
  }));
  let overviewRequestCount = 0;

  await page.route('/api/auth/status', async (route) => {
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify({
        setup_required: authState.setupRequired,
        authenticated: authState.authenticated,
        username: authState.authenticated ? authState.username : null,
      }),
    });
  });

  await page.route('/api/auth/me', async (route) => {
    if (authState.authenticated && authState.username) {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ username: authState.username }),
      });
      return;
    }

    await route.fulfill({
      status: 401,
      contentType: 'application/json',
      body: JSON.stringify({ error: 'unauthorized' }),
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

  await page.route('**/api/services**', async (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    const serviceName = decodeURIComponent(
      url.pathname.split('/').filter(Boolean).at(2) ?? '',
    );

    if (method === 'GET' && url.pathname === '/api/services') {
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          items: serviceState.map(
            ({ name, load, active, sub, description, unit_file_state }) => ({
              name,
              load,
              active,
              sub,
              description,
              unit_file_state,
            }),
          ),
        }),
      });
      return;
    }

    if (method === 'GET' && url.pathname.startsWith('/api/services/')) {
      if (url.pathname.endsWith('/logs')) {
        const service = serviceState.find(({ name }) => name === serviceName);
        await route.fulfill({
          contentType: 'application/json',
          body: JSON.stringify({ items: service?.logs ?? [] }),
        });
        return;
      }

      const service = serviceState.find(({ name }) => name === serviceName);
      if (!service) {
        await route.fulfill({
          status: 404,
          contentType: 'application/json',
          body: JSON.stringify({ error: { code: 'not_found', message: 'not found' } }),
        });
        return;
      }

      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify(service.details),
      });
      return;
    }

    if (method === 'POST' && url.pathname.startsWith('/api/services/')) {
      serviceActions.push(`${method} ${url.pathname}`);
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ ok: true }),
      });
      return;
    }

    await route.fallback();
  });

  await page.route('**/api/overview', async (route) => {
    if (options?.overviewFailure) {
      await route.fulfill({
        status: 503,
        contentType: 'application/json',
        body: JSON.stringify({
          error: { code: 'bad_request', message: 'overview unavailable' },
        }),
      });
      return;
    }

    const nextOverview =
      overviewStates[Math.min(overviewRequestCount, overviewStates.length - 1)] ?? overviewState;
    overviewRequestCount += 1;
    await route.fulfill({
      contentType: 'application/json',
      body: JSON.stringify(nextOverview),
    });
  });

  await page.route('**/api/files**', async (route) => {
    const url = new URL(route.request().url());
    const method = route.request().method();
    const pathname = url.pathname;
    const queryPath = normalizePath(url.searchParams.get('path') ?? fileRoots[0]);
    const isAllowed = (candidate: string) =>
      fileRoots.some(
        (root) =>
          candidate === root ||
          candidate.startsWith(root.endsWith('/') ? root : `${root}/`),
      );
    const childrenOf = (dir: string) =>
      sortFiles(
        fileState
          .filter((entry) => dirname(entry.path) === normalizePath(dir))
          .map((entry) => ({
            ...entry,
            name: basename(entry.path),
          })),
      );
    const getEntry = (candidate: string) =>
      fileState.find((entry) => entry.path === normalizePath(candidate));

    if (filesFailure && method === 'GET' && pathname === '/api/files') {
      await route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({
          error: { code: 'bad_request', message: 'files unavailable' },
        }),
      });
      return;
    }

    if (method === 'GET' && pathname === '/api/files') {
      const current = normalizePath(url.searchParams.get('path') ?? fileRoots[0]);
      if (!isAllowed(current)) {
        await route.fulfill({
          status: 403,
          contentType: 'application/json',
          body: JSON.stringify({
            error: { code: 'forbidden', message: 'forbidden' },
          }),
        });
        return;
      }

      const entry = getEntry(current);
      if (entry && entry.kind !== 'dir') {
        await route.fulfill({
          status: 400,
          contentType: 'application/json',
          body: JSON.stringify({
            error: { code: 'bad_request', message: 'path is not a directory' },
          }),
        });
        return;
      }

      const parent = normalizePath(path.posix.dirname(current));
      const parentPath =
        parent !== '.' && parent !== '/' && isAllowed(parent) ? parent : null;

      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          path: current,
          parent_path: parentPath,
          allowed_roots: visibleRoots,
          entries: childrenOf(current),
        }),
      });
      return;
    }

    if (method === 'GET' && pathname === '/api/files/preview') {
      const entry = getEntry(queryPath);
      if (!entry) {
        await route.fulfill({
          status: 404,
          contentType: 'application/json',
          body: JSON.stringify({
            error: { code: 'not_found', message: 'not found' },
          }),
        });
        return;
      }

      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          path: entry.path,
          size: entry.content?.length ?? entry.size ?? 0,
          truncated: false,
          content: entry.content ?? '',
        }),
      });
      return;
    }

    if (method === 'GET' && pathname === '/api/files/download') {
      const entry = getEntry(queryPath);
      if (!entry) {
        await route.fulfill({
          status: 404,
          contentType: 'application/json',
          body: JSON.stringify({
            error: { code: 'not_found', message: 'not found' },
          }),
        });
        return;
      }

      await route.fulfill({
        contentType: 'text/plain',
        body: entry.content ?? '',
      });
      return;
    }

    if (method === 'POST' && pathname === '/api/files/mkdir') {
      const body = JSON.parse(route.request().postData() ?? '{}') as {
        path?: string;
        name?: string;
      };
      const parent = normalizePath(body.path ?? '');
      const name = body.name ?? '';
      const target = normalizePath(path.posix.join(parent, name));
      fileState.push(
        makeFileFixture({
          path: target,
          kind: 'dir',
        }),
      );
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, path: target }),
      });
      return;
    }

    if (method === 'POST' && pathname === '/api/files/rename') {
      const body = JSON.parse(route.request().postData() ?? '{}') as {
        path?: string;
        new_name?: string;
      };
      const source = normalizePath(body.path ?? '');
      const target = normalizePath(path.posix.join(dirname(source), body.new_name ?? ''));
      fileState = updateDescendantPaths(fileState, source, target);
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, path: target }),
      });
      return;
    }

    if (method === 'DELETE' && pathname === '/api/files') {
      const body = JSON.parse(route.request().postData() ?? '{}') as {
        path?: string;
      };
      const target = normalizePath(body.path ?? '');
      const entry = getEntry(target);
      const descendants = fileState.filter(
        (item) => item.path.startsWith(`${target}/`) && item.path !== target,
      );
      if (entry?.kind === 'dir' && descendants.length > 0) {
        await route.fulfill({
          status: 409,
          contentType: 'application/json',
          body: JSON.stringify({
            error: { code: 'bad_request', message: 'directory is not empty' },
          }),
        });
        return;
      }
      fileState = fileState.filter(
        (item) => item.path !== target && !item.path.startsWith(`${target}/`),
      );
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({
          ok: true,
          parent_path: isAllowed(dirname(target)) ? dirname(target) : null,
        }),
      });
      return;
    }

    if (method === 'POST' && pathname === '/api/files/upload') {
      const bodyBuffer = route.request().postDataBuffer();
      const bodyText = bodyBuffer.toString('utf8');
      const filenameMatch = bodyText.match(/filename="([^"]+)"/);
      const contentMatch = bodyText.match(/\r\n\r\n([\s\S]*?)\r\n--/);
      const filename = filenameMatch?.[1] ?? 'upload.txt';
      const content = contentMatch?.[1] ?? '';
      const target = normalizePath(
        path.posix.join(queryPath, filename),
      );
      fileState.push(
        makeFileFixture({
          path: target,
          kind: 'file',
          size: content.length,
          content,
        }),
      );
      await route.fulfill({
        contentType: 'application/json',
        body: JSON.stringify({ ok: true, path: target }),
      });
      return;
    }

    await route.fallback();
  });

  await page.goto('/');
  if (authState.authenticated) {
    await expect(page.getByRole('heading', { name: 'Terminals' })).toBeVisible();
  }

  return {
    getServiceActions: () => serviceActions.slice(),
  };
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

test('fresh install shows initial setup', async ({ page, request }) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    auth: {
      setupRequired: true,
      authenticated: false,
      username: null,
    },
  });

  await expect(page.getByText('Initial setup')).toBeVisible();
  await expect(
    page.getByText('Create the first user for this host.'),
  ).toBeVisible();
  await expect(page.getByLabel('Username')).toHaveValue('');
  await expect(
    page.getByRole('button', { name: 'Create first user' }),
  ).toBeVisible();
});

test('existing user without a session shows login', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    auth: {
      setupRequired: false,
      authenticated: false,
      username: null,
    },
  });

  await expect(page.getByText('Sign in to manage this host.')).toBeVisible();
  await expect(
    page.getByText('Use your existing account to continue.'),
  ).toBeVisible();
  await expect(page.getByLabel('Username')).toHaveValue('');
  await expect(page.getByRole('button', { name: 'Sign in' })).toBeVisible();
});

test('invalid session shows login, not setup', async ({ page, request }) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    auth: {
      setupRequired: false,
      authenticated: false,
      username: null,
    },
  });

  await expect(page.getByText('Sign in to manage this host.')).toBeVisible();
  await expect(page.getByText('Initial setup')).toHaveCount(0);
});

test('services page opens and manages systemd units', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  const shell = await mockAuthenticatedShell(page, [], {
    services: [
      {
        name: 'homepanel.service',
        load: 'loaded',
        active: 'active',
        sub: 'running',
        description: 'HomePanel daemon',
        unit_file_state: 'enabled',
        details: {
          name: 'homepanel.service',
          load_state: 'loaded',
          active_state: 'active',
          sub_state: 'running',
          unit_file_state: 'enabled',
          description: 'HomePanel daemon',
          fragment_path: '/usr/lib/systemd/system/homepanel.service',
          main_pid: 1234,
          memory_current: 1048576,
          cpu_usage_nsec: 2048,
        },
        logs: ['2026-06-17 10:00:00 booted', '2026-06-17 10:01:00 ready'],
      },
      {
        name: 'ssh.service',
        load: 'loaded',
        active: 'active',
        sub: 'running',
        description: 'OpenSSH server',
        unit_file_state: 'enabled',
        details: {
          name: 'ssh.service',
          load_state: 'loaded',
          active_state: 'active',
          sub_state: 'running',
          unit_file_state: 'enabled',
          description: 'OpenSSH server',
          fragment_path: '/usr/lib/systemd/system/ssh.service',
          main_pid: 1400,
          memory_current: 222222,
          cpu_usage_nsec: 1024,
        },
        logs: ['2026-06-17 09:59:00 listening'],
      },
      {
        name: 'minecraft.service',
        load: 'loaded',
        active: 'inactive',
        sub: 'dead',
        description: 'Minecraft server',
        unit_file_state: 'enabled',
        details: {
          name: 'minecraft.service',
          load_state: 'loaded',
          active_state: 'inactive',
          sub_state: 'dead',
          unit_file_state: 'enabled',
          description: 'Minecraft server',
          fragment_path: '/usr/lib/systemd/system/minecraft.service',
          main_pid: 0,
          memory_current: null,
          cpu_usage_nsec: null,
        },
        logs: ['2026-06-17 09:58:00 stopped'],
      },
      {
        name: 'squad.service',
        load: 'loaded',
        active: 'inactive',
        sub: 'dead',
        description: 'Squad server',
        unit_file_state: 'disabled',
        details: {
          name: 'squad.service',
          load_state: 'loaded',
          active_state: 'inactive',
          sub_state: 'dead',
          unit_file_state: 'disabled',
          description: 'Squad server',
          fragment_path: '/usr/lib/systemd/system/squad.service',
          main_pid: 0,
          memory_current: null,
          cpu_usage_nsec: null,
        },
        logs: ['2026-06-17 09:57:00 stopped'],
      },
      {
        name: 'beammp.service',
        load: 'loaded',
        active: 'failed',
        sub: 'failed',
        description: 'BeamMP server',
        unit_file_state: 'enabled',
        details: {
          name: 'beammp.service',
          load_state: 'loaded',
          active_state: 'failed',
          sub_state: 'failed',
          unit_file_state: 'enabled',
          description: 'BeamMP server',
          fragment_path: '/usr/lib/systemd/system/beammp.service',
          main_pid: 0,
          memory_current: null,
          cpu_usage_nsec: null,
        },
        logs: ['2026-06-17 09:56:00 crashed'],
      },
      {
        name: 'nginx.service',
        load: 'loaded',
        active: 'active',
        sub: 'running',
        description: 'The NGINX HTTP and reverse proxy server',
        unit_file_state: 'disabled',
        details: {
          name: 'nginx.service',
          load_state: 'loaded',
          active_state: 'active',
          sub_state: 'running',
          unit_file_state: 'disabled',
          description: 'The NGINX HTTP and reverse proxy server',
          fragment_path: '/usr/lib/systemd/system/nginx.service',
          main_pid: 1888,
          memory_current: 3072000,
          cpu_usage_nsec: 4096,
        },
        logs: ['2026-06-17 09:55:00 running'],
      },
    ],
  });

  await page.getByRole('button', { name: 'Services', exact: true }).click();

  await expect(
    page
      .getByLabel('Systemd services')
      .getByRole('heading', { level: 2, name: 'Services' }),
  ).toBeVisible();
  await expect(page.getByTestId('service-search')).toBeVisible();
  await expect(page.getByRole('button', { name: 'All', exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Running', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Failed', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Enabled', exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Important', exact: true }),
  ).toBeVisible();
  await expect(page.getByTestId('service-count')).toContainText('6 of 6 services');
  const groupHeaders = page.getByTestId('service-list').locator('.service-group');
  await expect(groupHeaders.first()).toHaveAttribute(
    'data-testid',
    'service-group-important',
  );
  await expect(page.getByTestId('service-group-important')).toContainText('Important');
  await expect(page.getByTestId('service-group-other')).toContainText('Other services');
  await expect(page.getByTestId('service-status-homepanel-service')).toHaveText('RUN');
  await expect(page.getByTestId('service-status-ssh-service')).toHaveText('RUN');
  await expect(page.getByTestId('service-status-beammp-service')).toHaveText('FAIL');
  await expect(page.getByTestId('service-status-nginx-service')).toHaveText('RUN');

  await page.getByTestId('service-search').fill('minecraft');
  await expect(page.getByTestId('service-count')).toContainText('1 of 6 services');
  await expect(page.getByTestId('service-row-minecraft-service')).toBeVisible();
  await expect(page.getByTestId('service-row-ssh-service')).toHaveCount(0);

  await page.getByTestId('service-search').fill('');
  await page.getByRole('button', { name: 'Running', exact: true }).click();
  await expect(page.getByTestId('service-count')).toContainText('3 of 6 services');
  await expect(page.getByTestId('service-row-homepanel-service')).toBeVisible();
  await expect(page.getByTestId('service-row-ssh-service')).toBeVisible();
  await expect(page.getByTestId('service-row-nginx-service')).toBeVisible();
  await expect(page.getByTestId('service-status-nginx-service')).toHaveText('RUN');
  await expect(page.getByTestId('service-row-beammp-service')).toHaveCount(0);

  await page.getByRole('button', { name: 'Failed', exact: true }).click();
  await expect(page.getByTestId('service-count')).toContainText('1 of 6 services');
  await expect(page.getByTestId('service-row-beammp-service')).toBeVisible();
  await expect(page.getByTestId('service-status-beammp-service')).toHaveText('FAIL');
  await expect(page.getByTestId('service-row-homepanel-service')).toHaveCount(0);

  await page.getByRole('button', { name: 'Enabled', exact: true }).click();
  await expect(page.getByTestId('service-count')).toContainText('4 of 6 services');
  await expect(page.getByTestId('service-row-homepanel-service')).toBeVisible();
  await expect(page.getByTestId('service-row-ssh-service')).toBeVisible();
  await expect(page.getByTestId('service-row-minecraft-service')).toBeVisible();
  await expect(page.getByTestId('service-row-beammp-service')).toBeVisible();
  await expect(page.getByTestId('service-row-nginx-service')).toHaveCount(0);

  await page.getByRole('button', { name: 'Important', exact: true }).click();
  await expect(page.getByTestId('service-count')).toContainText('5 of 6 services');
  await expect(page.getByTestId('service-row-homepanel-service')).toBeVisible();
  await expect(page.getByTestId('service-row-ssh-service')).toBeVisible();
  await expect(page.getByTestId('service-row-minecraft-service')).toBeVisible();
  await expect(page.getByTestId('service-row-squad-service')).toBeVisible();
  await expect(page.getByTestId('service-row-beammp-service')).toBeVisible();
  await expect(page.getByTestId('service-row-nginx-service')).toHaveCount(0);

  await page.getByTestId('service-row-homepanel-service').click();
  await expect(page.getByTestId('service-details-panel')).toContainText(
    'homepanel.service',
  );
  await expect(page.getByTestId('service-logs')).toContainText('booted');

  await page.getByRole('button', { name: 'Restart' }).click();
  await expect(page.getByTestId('service-action-warning')).toContainText(
    'disconnect the panel',
  );
  await expect(page.getByTestId('service-action-summary')).toContainText(
    'homepanel.service',
  );
  await expect(page.getByRole('dialog')).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect.poll(() => shell.getServiceActions()).toEqual([]);

  await page.getByRole('button', { name: 'Restart' }).click();
  await page.getByRole('button', { name: 'Confirm' }).click();
  await expect.poll(() => shell.getServiceActions()).toContain(
    'POST /api/services/homepanel.service/restart',
  );
});

test('overview opens as a real dashboard', async ({ page, request }) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    services: [
      {
        name: 'homepanel.service',
        load: 'loaded',
        active: 'active',
        sub: 'running',
        description: 'HomePanel daemon',
        unit_file_state: 'enabled',
        details: {
          name: 'homepanel.service',
          load_state: 'loaded',
          active_state: 'active',
          sub_state: 'running',
          unit_file_state: 'enabled',
          description: 'HomePanel daemon',
          fragment_path: '/usr/lib/systemd/system/homepanel.service',
          main_pid: 1234,
          memory_current: 1048576,
          cpu_usage_nsec: 2048,
        },
        logs: ['2026-06-17 10:00:00 booted'],
      },
      {
        name: 'minecraft.service',
        load: 'loaded',
        active: 'inactive',
        sub: 'dead',
        description: 'Minecraft server',
        unit_file_state: 'enabled',
        details: {
          name: 'minecraft.service',
          load_state: 'loaded',
          active_state: 'inactive',
          sub_state: 'dead',
          unit_file_state: 'enabled',
          description: 'Minecraft server',
          fragment_path: '/usr/lib/systemd/system/minecraft.service',
          main_pid: 0,
          memory_current: null,
          cpu_usage_nsec: null,
        },
        logs: ['2026-06-17 09:58:00 stopped'],
      },
      {
        name: 'beammp.service',
        load: 'loaded',
        active: 'failed',
        sub: 'failed',
        description: 'BeamMP server',
        unit_file_state: 'enabled',
        details: {
          name: 'beammp.service',
          load_state: 'loaded',
          active_state: 'failed',
          sub_state: 'failed',
          unit_file_state: 'enabled',
          description: 'BeamMP server',
          fragment_path: '/usr/lib/systemd/system/beammp.service',
          main_pid: 0,
          memory_current: null,
          cpu_usage_nsec: null,
        },
        logs: ['2026-06-17 09:56:00 crashed'],
      },
    ],
    overview: {
      hostname: 'orchard',
      uptime_seconds: 3_665,
      load_average: [0.42, 0.31, 0.25],
      cpu_usage_percent: 0.33,
      memory_total_bytes: 8 * 1024 ** 3,
      memory_available_bytes: 5 * 1024 ** 3,
      memory_used_bytes: 3 * 1024 ** 3,
      disks: [
        {
          mount_point: '/',
          total_bytes: 100 * 1024 ** 3,
          used_bytes: 40 * 1024 ** 3,
          available_bytes: 60 * 1024 ** 3,
        },
        {
          mount_point: '/mnt/games',
          total_bytes: 500 * 1024 ** 3,
          used_bytes: 200 * 1024 ** 3,
          available_bytes: 300 * 1024 ** 3,
        },
      ],
      primary_ips: ['192.168.1.20', '10.10.0.5'],
      terminal_count: 0,
      service_summary: {
        total: 3,
        running: 1,
        failed: 1,
      },
      storage_path: '/var/lib/homepanel',
      database_path: '/var/lib/homepanel/homepanel.db',
      version: '0.1.0',
    },
    overviewSequence: [
      {
        cpu_usage_percent: 0.33,
      },
      {
        cpu_usage_percent: 23,
      },
    ],
  });

  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();
  await expect(page.getByTestId('dashboard-page')).toBeVisible();
  await expect(page.getByTestId('dashboard-hostname')).toHaveText('orchard');
  await expect(page.getByTestId('dashboard-summary')).toContainText('1h 1m');
  await expect(page.getByTestId('dashboard-summary')).toContainText(
    '192.168.1.20',
  );
  await expect(page.getByTestId('dashboard-summary')).toContainText('CPU 0.3%');
  await expect(page.getByTestId('dashboard-summary')).toContainText('RAM 3.00 GiB / 8.00 GiB');
  await expect(page.getByTestId('dashboard-summary')).not.toContainText('Load');
  await expect(page.getByTestId('dashboard-metric-load').locator('strong')).toHaveText(
    '0.3%',
  );
  await expect(page.getByTestId('dashboard-metric-load')).toContainText('Live usage');
  await expect(page.getByTestId('dashboard-metric-memory')).toContainText(
    '3.00 GiB / 8.00 GiB',
  );
  await expect(page.getByTestId('dashboard-metric-root-disk')).toContainText('40%');
  await expect(page.getByTestId('dashboard-metric-root-disk')).not.toContainText(
    'n/a',
  );
  await expect(page.getByTestId('dashboard-metric-games-disk')).toContainText('40%');
  await expect(page.getByTestId('dashboard-metric-terminals')).toContainText('0');
  await expect(page.getByTestId('dashboard-metric-failed-services')).toContainText(
    '1',
  );
  await expect(page.getByTestId('dashboard-health')).toContainText('Online');
  await expect(page.getByTestId('dashboard-health')).toContainText(
    '/var/lib/homepanel',
  );
  await expect(page.getByTestId('dashboard-mount-games')).toContainText(
    '/mnt/games',
  );
  await expect(page.getByTestId('dashboard-mount-root')).toContainText(
    '40.0 GiB / 100 GiB',
  );
  await expect(page.getByTestId('dashboard-storage')).not.toContainText(
    '/var/lib/homepanel',
  );
  await expect(page.getByTestId('dashboard-summary')).toContainText('CPU 23%');
  await expect(page.getByTestId('dashboard-metric-load').locator('strong')).toHaveText(
    '23%',
  );
  await expect(page.getByTestId('dashboard-summary')).not.toContainText('Load');

  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await expect(page.getByTestId('dashboard-page')).toBeVisible();

  await page.getByTestId('dashboard-action-terminals').click();
  await expect(page.getByRole('heading', { name: 'Terminals' })).toBeVisible();
  await page.getByRole('button', { name: 'Overview' }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();

  await page.getByTestId('dashboard-action-files').click();
  await expect(page.getByRole('heading', { name: 'Files' })).toBeVisible();
  await page.getByRole('button', { name: 'Overview' }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();

  await page.getByTestId('dashboard-action-services').click();
  await expect(
    page
      .getByLabel('Systemd services')
      .getByRole('heading', { name: 'Services' }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Overview' }).click();
  await expect(page.getByRole('heading', { name: 'Overview', exact: true })).toBeVisible();

  await page.getByTestId('dashboard-action-logs').click();
  await expect(page.getByRole('heading', { name: 'Logs' })).toBeVisible();
});

test('overview shows an error state when the api fails', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    overviewFailure: true,
  });

  await page.getByRole('button', { name: 'Overview', exact: true }).click();
  await expect(page.getByTestId('dashboard-page')).toBeVisible();
  await expect(page.getByTestId('dashboard-error')).toContainText(
    'overview unavailable',
  );
  await expect(page.getByTestId('dashboard-health')).toContainText('Offline');
  await expect(page.getByTestId('dashboard-metric-terminals')).toContainText(
    'n/a',
  );
});

test('files page behaves like an explorer', async ({
  page,
  request,
}) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    files: {
      roots: ['/home', '/mnt', '/mnt/games', '/srv', '/DATA'],
      visibleRoots: ['/home', '/mnt', '/mnt/games', '/srv'],
    },
  });

  await page.getByRole('button', { name: 'Files', exact: true }).click();
  const homeLocation = page.getByTestId('quick-location-home');
  const gamesLocation = page.getByTestId('quick-location-games');
  const serverLocation = page.getByTestId('quick-location-server-data');
  await expect(homeLocation).toBeVisible();
  await expect(gamesLocation).toBeVisible();
  await expect(serverLocation).toBeVisible();
  await expect(page.getByTestId('root-location-mnt')).toBeVisible();
  await expect(page.getByTestId('root-location-data')).toHaveCount(0);
  const homeBox = await homeLocation.boundingBox();
  expect(homeBox?.height ?? 0).toBeGreaterThan(0);
  expect(homeBox?.height ?? 0).toBeLessThan(48);
  await expect(page.getByTestId('files-topbar').getByTestId('files-address')).toBeVisible();
  await expect(page.getByTestId('files-topbar').getByTestId('files-search')).toBeVisible();
  await expect(page.getByTestId('files-address')).toHaveValue('/home');
  await expect(page.getByTestId('files-grid')).toBeVisible();
  await expect(page.getByTestId('files-table')).toHaveCount(0);

  await serverLocation.click();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv');
  await expect(page.getByTestId('file-tile-app')).toBeVisible();
  await expect(page.getByTestId('file-tile-logs')).toBeVisible();
  await expect(page.getByRole('link', { name: 'Download' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Rename' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Delete' })).toHaveCount(0);

  await page.getByTestId('files-search').fill('log');
  await expect(page.getByTestId('file-tile-logs')).toBeVisible();
  await expect(page.getByTestId('file-tile-app')).toHaveCount(0);
  await page.getByTestId('files-search').fill('');

  await page.getByTestId('file-tile-app').click();
  await expect(page.getByTestId('file-tile-app')).toHaveClass(/selected/);

  await page.getByTestId('file-tile-app').dblclick();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv/app');
  await expect(page.getByTestId('file-tile-config-txt')).toBeVisible();
  await expect(page.getByTestId('file-tile-readme-txt')).toBeVisible();

  await page.getByTestId('files-address').fill('/srv/app');
  await page.getByTestId('files-address').press('Enter');
  await expect(page.getByTestId('files-address')).toHaveValue('/srv/app');
  await expect(page.getByTestId('file-tile-config-txt')).toBeVisible();
  await expect(page.getByTestId('file-tile-readme-txt')).toBeVisible();

  await page.getByRole('button', { name: 'Back' }).click();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv');
  await page.getByRole('button', { name: 'Forward' }).click();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv/app');
  await page.getByTestId('files-topbar').getByRole('button', { name: 'Up' }).click();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv');

  await page.getByTestId('files-address').fill('/not-allowed');
  await page.getByTestId('files-address').press('Enter');
  await expect(page.getByRole('alert')).toContainText('forbidden');
  await expect(page.getByTestId('files-address')).toHaveValue('/srv');

  await page.getByTestId('root-location-mnt').click();
  await expect(page.getByTestId('files-address')).toHaveValue('/mnt');
  await expect(page.getByRole('alert')).toHaveCount(0);
  await expect(page.getByTestId('file-tile-games')).toBeVisible();

  await serverLocation.click();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv');

  await page.getByRole('button', { name: 'New folder' }).click();
  await page.getByLabel('Folder name').fill('sandbox');
  await page.getByRole('button', { name: 'Create folder' }).click();
  await expect(page.getByTestId('file-tile-sandbox')).toBeVisible();

  await page.getByTestId('file-tile-sandbox').click({ button: 'right' });
  const folderMenu = page.getByRole('menu', { name: 'File actions' });
  await expect(folderMenu).toContainText('Open');
  await expect(folderMenu).toContainText('Rename');
  await expect(folderMenu).toContainText('Delete');
  await expect(folderMenu.getByRole('link', { name: 'Download' })).toHaveCount(0);
  await folderMenu.getByRole('button', { name: 'Rename' }).click();
  await page.getByLabel('New name').fill('sandbox-renamed');
  await page
    .getByRole('dialog', { name: 'Rename item' })
    .getByRole('button', { name: 'Rename' })
    .click();
  await expect(page.getByTestId('file-tile-sandbox-renamed')).toBeVisible();

  await page.getByTestId('file-tile-sandbox-renamed').click({ button: 'right' });
  await page
    .getByRole('menu', { name: 'File actions' })
    .getByRole('button', { name: 'Delete' })
    .click();
  await page
    .getByRole('dialog', { name: 'Delete item' })
    .getByRole('button', { name: 'Delete' })
    .click();
  await expect(page.getByTestId('files-grid')).not.toContainText('sandbox-renamed');

  await page.getByTestId('file-tile-app').dblclick();
  await expect(page.getByTestId('files-address')).toHaveValue('/srv/app');
  await page.getByTestId('file-tile-readme-txt').click({ button: 'right' });
  const fileMenu = page.getByRole('menu', { name: 'File actions' });
  await expect(fileMenu).toContainText('Preview');
  await expect(fileMenu).toContainText('Download');
  await expect(fileMenu).toContainText('Rename');
  await expect(fileMenu).toContainText('Delete');
  await fileMenu.getByRole('button', { name: 'Preview' }).click();
  await expect(page.getByRole('dialog', { name: 'Text preview' })).toContainText(
    'Explorer test preview',
  );
  await expect(page.getByRole('link', { name: 'Download' })).toHaveAttribute(
    'href',
    /\/api\/files\/download\?path=/,
  );
  await page
    .getByRole('dialog', { name: 'Text preview' })
    .getByRole('button', { name: 'Close preview' })
    .click();

  await page.getByTestId('file-tile-readme-txt').click({ button: 'right' });
  await page.getByRole('button', { name: 'Rename' }).click();
  await page.getByLabel('New name').fill('readme-renamed.txt');
  await page
    .getByRole('dialog', { name: 'Rename item' })
    .getByRole('button', { name: 'Rename' })
    .click();
  await expect(page.getByTestId('file-tile-readme-renamed-txt')).toBeVisible();

  await page.getByTestId('file-tile-readme-renamed-txt').click({ button: 'right' });
  await page
    .getByRole('menu', { name: 'File actions' })
    .getByRole('button', { name: 'Delete' })
    .click();
  await page
    .getByRole('dialog', { name: 'Delete item' })
    .getByRole('button', { name: 'Delete' })
    .click();
  await expect(page.getByTestId('files-grid')).not.toContainText('readme-renamed.txt');

  await page.getByTestId('files-topbar').getByRole('button', { name: 'Up' }).click();
  await expect(page.getByRole('button', { name: 'Upload' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'New folder' })).toBeVisible();
  await page.getByRole('button', { name: 'Upload' }).click();
  await page.locator('input[type="file"]').setInputFiles({
    name: 'upload.txt',
    mimeType: 'text/plain',
    buffer: Buffer.from('uploaded from test\n'),
  });
  await expect(page.getByTestId('file-tile-upload-txt')).toBeVisible();
});

test('files page shows API failures clearly', async ({ page, request }) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [], {
    files: {
      failList: true,
    },
  });

  await page.getByRole('button', { name: 'Files', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('files unavailable');
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

test('authenticated session survives refresh', async ({ page, request }) => {
  await requireBackend(request);
  await mockAuthenticatedShell(page, [
    makeTerminal({ id: 'persist-terminal-0001', name: 'Persist Shell' }),
  ]);

  await expect(page.getByRole('heading', { name: 'Persist Shell' })).toBeVisible();
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Persist Shell' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Logout' })).toBeVisible();
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
