# Shelly — Next Steps

## Before First Release

### Signing & Distribution
- [ ] Generate Tauri signing keys on dev Mac with Apple Developer account:
  ```bash
  cargo tauri signer generate --password "your-password" -w ~/.tauri-keys/shelly.key
  ```
- [ ] Copy the **public key** into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`
- [ ] Update the `endpoints` URL to your actual GitHub repo (currently `anthropics/shelly`)
- [ ] Set up macOS code signing:
  ```bash
  export APPLE_SIGNING_IDENTITY="Developer ID Application: ..."
  export APPLE_ID="your@email.com"
  export APPLE_PASSWORD="app-specific-password"
  export APPLE_TEAM_ID="YOUR_TEAM_ID"
  ```
- [ ] Run `./scripts/publish.sh 1.0.0` to create first release
- [ ] Test auto-update: install v1.0.0, publish v1.0.1, verify it updates

### App Icon
- [ ] Replace default Tauri icons in `src-tauri/icons/` with custom Shelly icon
- [ ] Need: 32x32, 128x128, 128x128@2x, icon.icns, icon.ico, icon.png

### Git Setup
- [ ] Initialize git repo: `git init`
- [ ] Add `.gitignore` (node_modules, target, dist, .tauri-keys)
- [ ] Create GitHub repo
- [ ] Push initial commit

---

## Feature Roadmap

### High Priority
- [ ] **Codex CLI support** — Research Codex's hook/plugin system, add hook installation for Codex alongside Claude Code
- [ ] **Gemini CLI support** — Same as above for Google's Gemini CLI
- [ ] **Notification history** — Scrollable log of past events (stored in memory or localStorage)
- [ ] **Universal binary** — Build for both Apple Silicon and Intel, update `latest.json` with both platform entries

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
- [ ] **Analytics** — Track usage patterns (opt-in) to improve UX
- [ ] **Windows/Linux support** — Tauri supports all platforms, but transparent windows + hooks need platform-specific work

---

## Known Issues

- **Terminal detection removed** — Was too noisy (matching system processes). Could re-add with stricter detection using `NSWorkspace` API instead of `ps aux`
- **Ghost mode + queue** — If many events queue up in ghost mode, the feedback overlay plays for each one sequentially which can feel slow
- **Backdrop-filter on Tauri** — The frosted glass effect depends on `backdrop-filter` which requires `macos-private-api` and may not work on all macOS versions
- **Hook cleanup on crash** — If Shelly crashes (not normal quit), hooks remain in `~/.claude/settings.json`. Users can manually remove entries containing `localhost:21517`

---

## Technical Debt

- [ ] Remove unused `sysinfo` crate from Cargo.toml (sessions.rs uses `ps aux` directly)
- [ ] Remove unused `tower-http` crate (cors not used)
- [ ] The `AGENT_CONFIGS` is duplicated in renderer.ts (can't import from Rust types) — consider generating from a shared source
- [ ] Auto-update silently downloads and relaunches — should show a notification before restarting
