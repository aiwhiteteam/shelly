# Shelly — Project Guide

## Overview

Shelly is a native macOS app that provides a floating glass-style overlay for monitoring and controlling AI coding agents. Built with Tauri 2 (Rust + WebView), it intercepts Claude Code hooks to display permission requests, multi-choice questions, and notifications in a beautiful UI — so developers never leave their editor.

## Architecture

```
src-tauri/
├── Cargo.toml           # Rust deps: tauri, axum, tokio, serde, updater, process
├── tauri.conf.json      # Window config, updater endpoint, bundle settings
├── capabilities/        # Tauri permission capabilities
└── src/
    ├── main.rs          # Entry point
    ├── lib.rs           # Tauri setup, IPC commands, plugin registration
    ├── server.rs        # Axum HTTP server (:21517) — hook endpoints
    ├── hooks.rs         # Auto-install/uninstall hooks in ~/.claude/settings.json
    └── sessions.rs      # Agent process scanning

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
./scripts/publish.sh     # Build, sign, and create GitHub release
./scripts/publish.sh 1.1.0  # Same but bump version first
```

## Key Concepts

### Hook System
On startup, installs HTTP hooks into `~/.claude/settings.json`:
- `PreToolUse` (matcher: `AskUserQuestion`) → `/hooks/pre-tool-use` — multi-choice questions
- `PermissionRequest` → `/hooks/permission` — allow/deny (auto-allows AskUserQuestion)
- `Notification` → `/hooks/notification` — fire-and-forget
- `Stop` → `/hooks/stop` — session completion
- `/hooks/auto-allow` — for "Allow Always" rules

Hooks are removed on quit (normal close, Exit event, Ctrl+C/SIGTERM).

### Event Queue
Incoming events are queued. Only one shows at a time. After user responds, next event pops. Pending count shown in header badge.

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
Uses `tauri-plugin-updater`. Checks GitHub Releases endpoint 10s after launch, then every 4 hours. Downloads and relaunches automatically.

## Conventions

- Tauri events: `shelly://` prefix (notification, question, permission, stop)
- IPC commands: snake_case (`get_sessions`, `respond_question`)
- Server state: `OnceLock` global for cross-thread access
- Oneshot channels for blocking HTTP responses
- Frontend: `@tauri-apps/api` imports, bundled with esbuild
- CSS themes via class on `#container` (`theme-white`, `theme-dark`; no class = glass)

## Publishing

See `scripts/publish.sh`. Requires:
1. Tauri signing keys (`cargo tauri signer generate`)
2. `TAURI_SIGNING_PRIVATE_KEY` env var
3. GitHub CLI (`gh`) for creating releases
4. Update `pubkey` in `tauri.conf.json` with your public key
5. Update `endpoints` URL to your actual GitHub repo
