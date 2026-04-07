use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::process::Command;

#[derive(Serialize)]
pub struct AgentSession {
    agent: String,
    pid: u32,
    cwd: String,
    session_id: String,
}

const TERMINAL_APPS: &[&str] = &[
    "iTerm2", "Terminal", "Warp", "Ghostty", "Alacritty",
    "kitty", "WezTerm", "Hyper", "Visual Studio Code",
    "Cursor", "Windsurf", "Zed", "Rio",
];

pub fn scan_all() -> Vec<AgentSession> {
    let mut sessions = scan_claude_sessions();
    sessions.extend(scan_process_agents());
    sessions
}

fn scan_claude_sessions() -> Vec<AgentSession> {
    let mut sessions = Vec::new();
    let sessions_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("sessions");

    if !sessions_dir.exists() {
        return sessions;
    }

    let entries = match fs::read_dir(&sessions_dir) {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(data) = serde_json::from_str::<Value>(&content) {
                    if let Some(pid) = data.get("pid").and_then(|v| v.as_u64()) {
                        // Check if process is alive
                        if is_process_alive(pid as u32) {
                            sessions.push(AgentSession {
                                agent: "claude-code".to_string(),
                                pid: pid as u32,
                                cwd: data.get("cwd").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
                                session_id: data.get("sessionId").and_then(|v| v.as_str())
                                    .unwrap_or(&path.file_stem().unwrap_or_default().to_string_lossy())
                                    .to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    sessions
}

fn scan_process_agents() -> Vec<AgentSession> {
    let mut sessions = Vec::new();
    let agents = [
        ("codex-cli", &["codex"][..]),
        ("gemini-cli", &["gemini"][..]),
        ("cursor", &["Cursor.app", "cursor --"][..]),
        ("opencode", &["opencode"][..]),
    ];

    let output = match Command::new("ps").args(["aux"]).output() {
        Ok(o) => String::from_utf8_lossy(&o.stdout).to_string(),
        Err(_) => return sessions,
    };

    let mut seen_pids = std::collections::HashSet::new();

    for line in output.lines() {
        for (agent_id, proc_names) in &agents {
            for proc_name in *proc_names {
                if line.to_lowercase().contains(proc_name) && !line.contains("grep") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(pid_str) = parts.get(1) {
                        if let Ok(pid) = pid_str.parse::<u32>() {
                            if seen_pids.insert(pid) {
                                sessions.push(AgentSession {
                                    agent: agent_id.to_string(),
                                    pid,
                                    cwd: "unknown".to_string(),
                                    session_id: format!("{}-{}", agent_id, pid),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    sessions
}

fn is_process_alive(pid: u32) -> bool {
    // signal 0 = existence check
    unsafe { nix_kill(pid as i32, 0) == 0 }
}

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

unsafe fn nix_kill(pid: i32, sig: i32) -> i32 {
    unsafe { kill(pid, sig) }
}

/// Get the controlling TTY of a process (e.g. "ttys001"), if any.
fn tty_of_pid(pid: u32) -> Option<String> {
    let output = Command::new("ps")
        .args(["-o", "tty=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    let tty = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if tty.is_empty() || tty == "??" || tty == "?" {
        None
    } else {
        Some(tty)
    }
}

/// Walk the parent PID chain to find which terminal app a process is running in.
/// Returns (terminal name, optional tty of the agent process for tab targeting).
pub fn find_terminal_for_session(session_id: &str) -> Option<(String, Option<String>)> {
    // Find the PID for this session — try matching session_id, then try as PID filename
    let sessions = scan_all();
    let session = sessions.iter().find(|s| {
        s.session_id == session_id || s.pid.to_string() == session_id
    });

    // If no session match, try reading the session file directly by scanning ~/.claude/sessions/
    let pid = if let Some(s) = session {
        s.pid
    } else {
        // Try to find a session file where sessionId matches
        let sessions_dir = dirs::home_dir()?.join(".claude").join("sessions");
        let mut found_pid = None;
        if let Ok(entries) = fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "json").unwrap_or(false) {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                            let file_session_id = data.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
                            if file_session_id == session_id {
                                found_pid = data.get("pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                                break;
                            }
                        }
                    }
                }
            }
        }
        found_pid?
    };
    let agent_tty = tty_of_pid(pid);
    let mut pid = pid;

    // Known terminal app markers in process paths (lowercase)
    let markers: &[(&str, &str)] = &[
        ("iterm", "iTerm2"),
        ("terminal.app", "Terminal"),
        ("warp.app", "Warp"),
        ("ghostty", "Ghostty"),
        ("alacritty", "Alacritty"),
        ("kitty", "kitty"),
        ("wezterm", "WezTerm"),
        ("hyper", "Hyper"),
        ("visual studio code", "Visual Studio Code"),
        ("cursor.app", "Cursor"),
        ("windsurf", "Windsurf"),
        ("zed.app", "Zed"),
        ("rio.app", "Rio"),
    ];

    // Walk up the process tree (max 15 hops)
    for _ in 0..15 {
        let output = Command::new("ps")
            .args(["-o", "ppid=,comm=", "-p", &pid.to_string()])
            .output()
            .ok()?;
        let line = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if line.is_empty() {
            break;
        }

        let lower = line.to_lowercase();
        for (marker, app_name) in markers {
            if lower.contains(marker) {
                return Some((app_name.to_string(), agent_tty));
            }
        }

        // Move to parent
        let ppid: u32 = line.split_whitespace().next()?.parse().ok()?;
        if ppid <= 1 {
            break;
        }
        pid = ppid;
    }

    None
}

pub fn detect_terminals() -> Vec<String> {
    let mut running = Vec::new();
    let pattern = TERMINAL_APPS.join("|");
    let output = Command::new("sh")
        .args(["-c", &format!("ps aux | grep -i -E '{}' | grep -v grep", pattern)])
        .output();

    if let Ok(output) = output {
        let text = String::from_utf8_lossy(&output.stdout).to_lowercase();
        for app in TERMINAL_APPS {
            if text.contains(&app.to_lowercase()) {
                running.push(app.to_string());
            }
        }
    }

    running
}

pub fn jump_to_terminal(terminal_app: &str, tty: Option<&str>) {
    // For tabbed terminals with a known TTY, target the specific tab/session.
    if let Some(tty) = tty {
        let tty_path = if tty.starts_with("/dev/") { tty.to_string() } else { format!("/dev/{}", tty) };
        log::info!("jump_to_terminal: app={} tty_path={}", terminal_app, tty_path);
        let script = match terminal_app {
            "iTerm2" => Some(format!(
                r#"tell application "iTerm"
  activate
  repeat with w in windows
    repeat with t in tabs of w
      repeat with s in sessions of t
        if tty of s is "{}" then
          select w
          select t
          select s
          return
        end if
      end repeat
    end repeat
  end repeat
end tell"#,
                tty_path
            )),
            "Terminal" => Some(format!(
                r#"tell application "Terminal"
  activate
  repeat with w in windows
    repeat with t in tabs of w
      if tty of t is "{}" then
        set selected of t to true
        set index of w to 1
        return
      end if
    end repeat
  end repeat
end tell"#,
                tty_path
            )),
            _ => None,
        };
        if let Some(script) = script {
            let out = Command::new("osascript").args(["-e", &script]).output();
            match out {
                Ok(o) => log::info!(
                    "osascript exit={:?} stdout={} stderr={}",
                    o.status.code(),
                    String::from_utf8_lossy(&o.stdout),
                    String::from_utf8_lossy(&o.stderr)
                ),
                Err(e) => log::error!("osascript spawn failed: {}", e),
            }
            return;
        }
    }

    // Try activation by bundle ID first (more reliable), fall back to app name
    let activated = match terminal_app {
        "Visual Studio Code" => try_activate_bundle_id("com.microsoft.VSCode"),
        "Cursor" => try_activate_bundle_id("com.todesktop.230313mzl4w4u92"),
        _ => false,
    };

    if !activated {
        let app_name = match terminal_app {
            "iTerm2" => "iTerm",
            other => other,
        };
        let _ = Command::new("osascript")
            .args(["-e", &format!("tell application \"{}\" to activate", app_name)])
            .output();
    }
}

fn try_activate_bundle_id(bundle_id: &str) -> bool {
    Command::new("open")
        .args(["-b", bundle_id])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
