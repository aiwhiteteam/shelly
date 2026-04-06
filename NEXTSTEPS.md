# Shelly — Next Steps

## Before First Release

### Signing & Distribution
- [ ] Generate Tauri signing keys on dev Mac with Apple Developer account:
  ```bash
  cargo tauri signer generate --password "your-password" -w ~/.tauri-keys/shelly.key
  ```
- [ ] Copy the **public key** into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
- [ ] Update the `endpoints` URL to your actual GitHub repo
- [ ] Set up macOS code signing
- [ ] Run `./scripts/publish.sh 1.0.0` to create first release
- [ ] Test auto-update: install v1.0.0, publish v1.0.1, verify it updates

### App Icon
- [ ] Replace default Tauri icons in `src-tauri/icons/` with custom Shelly icon

---

## Feature Roadmap

### High Priority
- [ ] **Notification history** — Scrollable log of past events (stored in memory or localStorage)
- [ ] **Universal binary** — Build for both Apple Silicon and Intel

### Medium Priority
- [ ] **Custom sound packs** — Let users pick between sound themes (8-bit, minimal, macOS native, silent)
- [ ] **Configurable position** — Remember window position between launches
- [ ] **Configurable hotkeys** — Let users rebind ⌘Y/⌘N
- [ ] **Multiple question support** — Currently only renders first question from AskUserQuestion, should handle all questions in the array
- [ ] **Landing page / website** — Marketing site for distribution

### Low Priority
- [ ] **System tray icon** — Add menu bar icon as alternative to dock icon
- [ ] **Login at startup** — Launch Shelly automatically on macOS login
- [ ] **Notification center integration** — Send macOS native notifications as fallback when window is hidden
- [ ] **Windows/Linux support** — Tauri supports all platforms, but transparent windows + hooks need platform-specific work

---

## Completed

- [x] **Codex CLI support** — Command hooks via bridge script in `~/.codex/hooks.json`
- [x] **Gemini CLI support** — Command hooks via bridge script in `~/.gemini/settings.json`
- [x] **Jump to terminal** — Auto-detects terminal app via parent PID chain, supports iTerm2, Terminal.app, VS Code, Cursor
- [x] **Project context** — Shows project folder name on permission/question views
- [x] **Pending event rotation** — Click pending badge to cycle through queued events
- [x] **Stale request cleanup** — Background task dismisses events answered in terminal
- [x] **VS Code/Cursor redirect** — Uses `open -b` for reliable activation

---

## Known Issues

- **Ghost mode + queue** — If many events queue up in ghost mode, the feedback overlay plays for each one sequentially which can feel slow
- **Backdrop-filter on Tauri** — The frosted glass effect depends on `backdrop-filter` which requires `macos-private-api` and may not work on all macOS versions
- **Hook cleanup on crash** — If Shelly crashes (not normal quit), hooks remain in config files. Users can manually remove entries containing `localhost:21517` or `shelly-bridge.py`

---

## Technical Debt

- [ ] The `AGENT_CONFIGS` is duplicated in renderer.ts (can't import from Rust types) — consider generating from a shared source
- [ ] Auto-update silently downloads and relaunches — should show a notification before restarting
