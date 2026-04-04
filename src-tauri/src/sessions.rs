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

pub fn jump_to_terminal(terminal_app: &str) {
    let app_name = match terminal_app {
        "Terminal" => "Terminal",
        "iTerm2" => "iTerm",
        "Visual Studio Code" => "Visual Studio Code",
        other => other,
    };

    let _ = Command::new("osascript")
        .args(["-e", &format!("tell application \"{}\" to activate", app_name)])
        .output();
}
