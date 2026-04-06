# Shelly — Project Guide

## Overview

Shelly is a native macOS app that provides a floating glass-style overlay for monitoring and controlling AI coding agents. Built with Tauri 2 (Rust + WebView), it intercepts hooks from Claude Code, Codex CLI, and Gemini CLI to display permission requests, multi-choice questions, and notifications in a beautiful UI — so developers never leave their editor.

## Architecture

```
src-tauri/
├── Cargo.toml           # Rust deps: tauri, axum, tokio, serde, updater, process
├── tauri.conf.json      # Window config, updater endpoint, bundle settings
├── capabilities/        # Tauri permission capabilities
├── resources/
│   └── shelly-bridge.py # Bridge script for Codex/Gemini command hooks → HTTP
└── src/
    ├── main.rs          # Entry point
    ├── lib.rs           # Tauri setup, IPC commands, plugin registration
    ├── server.rs        # Axum HTTP server (:21517) — hook endpoints + stale cleanup
    ├── hooks.rs         # Auto-install/uninstall hooks for Claude/Codex/Gemini
    └── sessions.rs      # Agent process scanning, terminal detection, jump-to-terminal

src/renderer/
├── index.html           # UI markup
├── styles.css           # 3 themes: liquid glass, white, dark
└── renderer.ts          # Frontend logic, sounds, queue, ghost mode, auto-update
```

## Tech Stack

- **Framework**: Tauri 2.10 (Rust + system WebView)
- **Backend**: Rust — Axum (async HTTP), Tokio runtime
- **Frontend**: TypeScript (esbuild bundled), vanilla HTML/CSS
- **Audio**: Web Audio API (8-bit synthesized)
- **Auto-Update**: tauri-plugin-updater via GitHub Releases

## Commands

```bash
npm run dev              # Build frontend + cargo tauri dev
npm run build            # Production build (dmg + app)
npm run build:frontend   # Compile TS + copy assets only
cargo test               # Run Rust unit tests (37 tests)
./scripts/publish.sh     # Build, sign, and create GitHub release
./scripts/publish.sh 1.1.0  # Same but bump version first
```

## Key Concepts

### Multi-Agent Hook System
On startup, installs hooks for three agents:

**Claude Code** — HTTP hooks in `~/.claude/settings.json`:
- `PreToolUse` (matcher: `AskUserQuestion`) → `/hooks/pre-tool-use` — multi-choice questions
- `PermissionRequest` → `/hooks/permission` — allow/deny (auto-allows AskUserQuestion)
- `Notification` → `/hooks/notification` — fire-and-forget
- `Stop` → `/hooks/stop` — session completion
- `/hooks/auto-allow` — for "Allow Always" rules

**Codex CLI** — command hooks in `~/.codex/hooks.json` via `shelly-bridge.py`:
- `PreToolUse` → bridge → `/hooks/permission`
- `Stop` → bridge → `/hooks/stop`

**Gemini CLI** — command hooks in `~/.gemini/settings.json` via `shelly-bridge.py`:
- `BeforeTool` → bridge → `/hooks/permission`
- `Notification` → bridge → `/hooks/notification`
- `SessionEnd` → bridge → `/hooks/stop`

The bridge script (`~/.shelly/shelly-bridge.py`) translates stdin/stdout JSON to HTTP and back.

Hooks are removed on quit (normal close, Exit event, Ctrl+C/SIGTERM).

### Event Queue
Incoming events are queued. Only one shows at a time. After user responds, next event pops. Pending count shown in header badge. Clicking the pending badge rotates to the next event (re-queues current).

### Jump to Terminal
"Go to ↗" button on permission, question, notification, and stop views. Auto-detects which terminal app the agent is running in by walking the parent PID chain. Supports iTerm2, Terminal.app, VS Code (via `open -b`), Cursor, Warp, Ghostty, and others.

### Project Context
Permission and question views show the project folder name (e.g. `QUESTION · my-app`) looked up from the session's working directory.

### Stale Request Cleanup
Background task runs every 2s to detect when the agent drops the HTTP connection (user answered in terminal). Dismisses stale events from the frontend queue automatically.

### Ghost Mode
Toggle via 👻 button. When on:
- Window hides after responding (with vanish animation)
- Pops back on new events (with appear animation)
- Shows feedback overlay: ✔ ALLOWED (green), ✘ DENIED (red), ✔ ANSWERED (blue)

### Permission Response
Three options: Yes (allow once), Always (allow + add PreToolUse auto-allow rule), No (deny).

### Themes
Three themes cycling on button click:
- **◇ Glass** — frosted blur, shimmer sweep, specular breathing, chromatic edge dispersion
- **○ White** — solid #ffffff, no effects
- **● Dark** — solid #1a1a1a, no effects

### Auto-Update
Uses `tauri-plugin-updater`. Checks GitHub Releases endpoint 60s after launch. Downloads and relaunches automatically.

## Conventions

- Tauri events: `shelly://` prefix (notification, question, permission, stop, dismiss)
- IPC commands: snake_case (`get_sessions`, `respond_question`, `jump_to_session`)
- Server state: `OnceLock` global for cross-thread access
- Oneshot channels for blocking HTTP responses
- Frontend: `@tauri-apps/api` imports, bundled with esbuild
- CSS themes via class on `#container` (`theme-white`, `theme-dark`; no class = glass)
- Hook detection: HTTP hooks checked by `localhost:21517` in URL, command hooks by `shelly-bridge.py` in command

## Publishing

See `scripts/publish.sh`. Requires:
1. Tauri signing keys (`cargo tauri signer generate`)
2. `TAURI_SIGNING_PRIVATE_KEY` env var
3. GitHub CLI (`gh`) for creating releases
4. Update `pubkey` in `tauri.conf.json` with your public key
5. Update `endpoints` URL to your actual GitHub repo
