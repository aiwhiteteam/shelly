# Shelly

Shelly is a free macOS app that can monitor and auto approve all claude code sessions in all terminals. It intercepts permission requests, multi-choice questions, and notifications so you never have to switch to the terminal.

Built with Tauri 2 and Rust. Under 15MB, minimal RAM, instant startup.

<p align="center">
  <img src="assets/demo.png" alt="Shelly — floating overlay for AI coding agents" width="720" />
</p>

---

## Supported Agents

| Agent | Hook Type | Config File |
|-------|-----------|-------------|
| **Claude Code** | HTTP | `~/.claude/settings.json` |
| **OpenAI Codex CLI** | Command (via bridge) | `~/.codex/hooks.json` |
| **Google Gemini CLI** | Command (via bridge) | `~/.gemini/settings.json` |

Hooks are auto-installed on launch and cleanly removed on quit. No manual setup.

---

## Features

### Multi-Choice Question Answering
When your agent asks a question, Shelly intercepts it and shows the actual options as clickable buttons. Select by clicking or pressing number keys (1-9). Your answer goes back directly — the question never appears in the terminal.

### Permission Approvals
Three-button permission dialog:
- **Yes** (`⌘Y`) — allow this once
- **Always** — allow and never ask again for this tool
- **No** (`⌘N`) — deny

### Jump to Terminal
Every event view has a **Go to ↗** button that switches to the terminal where the agent is running. Auto-detects the correct app (iTerm2, Terminal.app, VS Code, Cursor, Warp, etc.) by walking the process tree.

### Project Context
Permission and question views show which project the event is from (e.g. `QUESTION · my-app`), so you always know which session needs attention.

### Pending Event Rotation
When multiple events are queued, click the **pending badge** in the header to cycle through them — the current event goes to the back of the queue.

### Ghost Mode
Toggle the 👻 button to enable ghost mode. The window hides after you respond and only pops back when there's something new. Includes animated feedback overlays showing ✔ ALLOWED, ✘ DENIED, or ✔ ANSWERED before fading away.

### Three Themes
- **Liquid Glass** — frosted translucent blur with shimmer animation, specular highlights, and chromatic edge dispersion
- **White** — clean solid white
- **Dark** — solid dark background

### 8-Bit Sound Alerts
Synthesized sound effects for notifications, permissions, questions, completions, and allow/deny responses. Mute with one click.

### Event Queue
Multiple incoming events are queued and shown one at a time. A badge shows how many are pending. Stale events (answered in terminal) are automatically cleaned up.

### Auto-Update
Checks for updates on launch and installs them automatically. No manual downloads needed after initial install.

### Privacy First
Server listens on `127.0.0.1` only. No data leaves your machine.

---

## Installation

### Prerequisites
- macOS (Apple Silicon or Intel)
- Rust toolchain (`rustup`)
- Tauri CLI (`cargo install tauri-cli`)
- Node.js 18+

### From Source

```bash
git clone <repo-url> shelly
cd shelly
npm install
cargo tauri dev      # Development
cargo tauri build    # Production (.dmg)
```

### From DMG
Download the latest `.dmg` from [Releases](https://github.com/aiwhiteteam/shelly/releases), open it, and drag Shelly to Applications.

> [**Download Shelly (macOS Apple Silicon)**](https://github.com/aiwhiteteam/shelly/releases/download/v1.2.2/Shelly_1.2.2_aarch64.dmg)

---

## How It Works

1. Shelly creates a frameless overlay at the top of your screen and starts an HTTP server on port 21517
2. Hooks are installed in agent config files (Claude, Codex, Gemini) so events are sent to Shelly
3. For Claude Code: hooks communicate directly via HTTP
4. For Codex/Gemini: a Python bridge script (`~/.shelly/shelly-bridge.py`) translates stdin/stdout JSON to HTTP
5. `PreToolUse` hook intercepts `AskUserQuestion` — shows multi-choice UI, sends answer back via `updatedInput`
6. `PermissionRequest` hook shows Yes/Always/No for tool approvals
7. Events queue up and show one at a time
8. Stale events (answered in terminal) are automatically dismissed
9. On quit, hooks are removed from all config files

---

## Architecture

```
┌─────────────────────────────────────┐
│         Shelly UI (WebView)         │
│  SHELLY ●  [i] [🔈] [◇] [👻] [✕]  │
│  ─────────────────────────────────  │
│  QUESTION · my-app                  │
│  Which framework?     [Go to ↗]    │
│  [1] React  [2] Vue  [3] Angular   │
└──────────────┬──────────────────────┘
               │ Tauri IPC
┌──────────────┴──────────────────────┐
│     Rust Backend (Tauri + Axum)     │
│  POST /hooks/pre-tool-use           │
│  POST /hooks/permission             │
│  POST /hooks/notification           │
│  POST /hooks/stop                   │
│  POST /hooks/auto-allow             │
└──────────┬─────────────┬────────────┘
           │ HTTP        │ stdin/stdout
┌──────────┴──┐  ┌───────┴────────────┐
│ Claude Code │  │ shelly-bridge.py   │
│             │  │  ↕ Codex / Gemini  │
└─────────────┘  └────────────────────┘
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `⌘Y` | Allow permission |
| `⌘N` | Deny permission |
| `1`–`9` | Select question option |

---

## Publishing Updates

```bash
# One-time: generate signing keys
cargo tauri signer generate --password "your-password" -w ~/.tauri-keys/shelly.key

# Set env vars
export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri-keys/shelly.key)
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="your-password"

# Publish a new version
./scripts/publish.sh 1.1.0
```

See `scripts/publish.sh` for the full workflow (build, sign, GitHub release, update manifest).

---

## Development

```bash
npm run build:frontend    # Build TS + copy HTML/CSS
cargo tauri dev           # Run with hot reload
cargo tauri build         # Production build
cargo test                # Run unit tests (37 tests)
```

### Project Structure

```
src-tauri/src/
├── main.rs           # Entry point
├── lib.rs            # Tauri setup, IPC commands, plugins
├── server.rs         # Axum HTTP server + stale request cleanup
├── hooks.rs          # Hook install/uninstall for Claude/Codex/Gemini
└── sessions.rs       # Agent process detection, terminal jump

src-tauri/resources/
└── shelly-bridge.py  # Bridge: Codex/Gemini command hooks → Shelly HTTP

src/renderer/
├── index.html        # UI markup
├── styles.css        # Themes + animations
└── renderer.ts       # All frontend logic
```

---

## License

MIT
