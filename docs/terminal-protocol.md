# Terminal Protocol

The terminal WebSocket protocol is JSON-based and mirrors the spec:

- client messages: `input`, `resize`, `ping`, `clear_scrollback`
- server messages: `hello`, `scrollback`, `output`, `status`, `exit`, `error`

The current scaffold serializes the shared protocol in `homepanel-core`.
