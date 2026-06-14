# Architecture

`homepanel` is split into:

- `homepaneld` for web/API/auth/static hosting
- `homepanel-agent` for PTY-backed terminal sessions
- `homepanel-core` for shared domain types and config

The first implementation keeps the code boundaries clean even though the agent is still linked in-process for simplicity during development.
