# homepanel

Rust home/server control panel with a native PTY-backed terminal agent, SQLite metadata, and a Svelte frontend.

This repository is scaffolded around the spec for:

- `homepaneld`
- `homepanel-agent`
- `homepanel-web`

## Layout

- `crates/homepanel-core`: shared types, config, terminal protocol, scrollback
- `crates/homepaneld`: HTTP/API daemon
- `crates/homepanel-agent`: PTY session manager
- `frontend`: Svelte + Vite SPA

## Development

Backend:

```bash
cargo build
cargo test
cargo run -p homepaneld
cargo run -p homepanel-agent
```

Frontend:

```bash
cd frontend
npm install
npm run dev
```

The current implementation focuses on the core architecture and a working foundation for:

- config loading
- auth/session scaffolding
- terminal session types and protocol
- PTY session manager structure
- basic SPA shell
