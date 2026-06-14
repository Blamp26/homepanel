# Frontend Checks

Run static checks before manual visual review:

```bash
npm run check
npm run lint
npm run build
```

Playwright tests start the Vite frontend server, but they expect `homepaneld` to be running because `/api` is proxied to `http://127.0.0.1:8080`.

```bash
cd /home/superadmin/projects/homepanel
cargo run -p homepaneld
```

Then, in another terminal:

```bash
cd /home/superadmin/projects/homepanel/frontend
npm run test:e2e
```

If Playwright browsers are not installed yet, run:

```bash
npx playwright install
```

The visual smoke test mocks authenticated frontend API responses after confirming the backend health endpoint is reachable. This keeps the screenshot useful without storing test credentials.
