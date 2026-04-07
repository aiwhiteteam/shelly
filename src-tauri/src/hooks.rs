use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const SHELLY_MARKER: &str = "localhost:21517";
const BRIDGE_MARKER: &str = "shelly-bridge.py";

// --- Path helpers ---

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

fn claude_settings_path() -> PathBuf {
    home_dir().join(".claude").join("settings.json")
}

fn codex_hooks_path() -> PathBuf {
    home_dir().join(".codex").join("hooks.json")
}

fn gemini_settings_path() -> PathBuf {
    home_dir().join(".gemini").join("settings.json")
}

fn bridge_script_path() -> PathBuf {
    home_dir().join(".shelly").join("shelly-bridge.py")
}

// --- JSON file I/O ---

fn read_json(path: &PathBuf) -> Value {
    if !path.exists() {
        return Value::Object(Default::default());
    }
    fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Object(Default::default()))
}

fn write_json(path: &PathBuf, value: &Value) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(value).unwrap_or_default();
    let _ = fs::write(path, format!("{}\n", json));
}

// --- Hook detection ---

fn is_shelly_hook(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                // HTTP hooks (Claude Code): check url contains localhost:21517
                let is_http = h.get("url")
                    .and_then(|u| u.as_str())
                    .map(|u| u.contains(SHELLY_MARKER))
                    .unwrap_or(false);
                // Command hooks (Codex/Gemini): check command contains shelly-bridge.py
                let is_cmd = h.get("command")
                    .and_then(|c| c.as_str())
                    .map(|c| c.contains(BRIDGE_MARKER))
                    .unwrap_or(false);
                is_http || is_cmd
            })
        })
        .unwrap_or(false)
}

// --- Hook definitions ---

fn claude_hooks() -> Value {
    serde_json::json!({
        "PreToolUse": [
            {
                "matcher": "AskUserQuestion",
                "hooks": [{
                    "type": "http",
                    "url": format!("http://{}/hooks/pre-tool-use", SHELLY_MARKER),
                    "timeout": 120
                }]
            }
        ],
        "Notification": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "http",
                    "url": format!("http://{}/hooks/notification", SHELLY_MARKER),
                    "timeout": 5,
                    "async": true
                }]
            }
        ],
        "PermissionRequest": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "http",
                    "url": format!("http://{}/hooks/permission", SHELLY_MARKER),
                    "timeout": 120
                }]
            }
        ],
        "Stop": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "http",
                    "url": format!("http://{}/hooks/stop", SHELLY_MARKER),
                    "timeout": 5,
                    "async": true
                }]
            }
        ]
    })
}

fn bridge_cmd(agent: &str, endpoint: &str) -> String {
    let script = bridge_script_path().display().to_string();
    format!("python3 {} {} {}", script, agent, endpoint)
}

// NOTE: Codex CLI and Gemini CLI hook `timeout` is specified in
// MILLISECONDS (unlike Claude Code, which uses seconds). 300000 = 5 min
// for blocking permission hooks, 30000 = 30s for fire-and-forget.

fn codex_hooks() -> Value {
    serde_json::json!({
        "PreToolUse": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": bridge_cmd("codex-cli", "permission"),
                    "timeout": 300000
                }]
            }
        ],
        "Stop": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": bridge_cmd("codex-cli", "stop"),
                    "timeout": 30000
                }]
            }
        ]
    })
}

fn gemini_hooks() -> Value {
    serde_json::json!({
        "BeforeTool": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": bridge_cmd("gemini-cli", "permission"),
                    "timeout": 300000
                }]
            }
        ],
        "Notification": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": bridge_cmd("gemini-cli", "notification"),
                    "timeout": 30000
                }]
            }
        ],
        "SessionEnd": [
            {
                "matcher": "",
                "hooks": [{
                    "type": "command",
                    "command": bridge_cmd("gemini-cli", "stop"),
                    "timeout": 30000
                }]
            }
        ]
    })
}

// --- Generic install/uninstall helpers ---

fn install_hooks_to_file(path: &PathBuf, hook_defs: &Value) {
    let mut settings = read_json(path);
    let hooks_obj = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));

    for (hook_name, shelly_entries) in hook_defs.as_object().unwrap() {
        let existing = hooks_obj
            .get(hook_name)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let non_shelly: Vec<Value> = existing
            .into_iter()
            .filter(|e| !is_shelly_hook(e))
            .collect();

        let mut merged = non_shelly;
        if let Some(arr) = shelly_entries.as_array() {
            merged.extend(arr.clone());
        }

        hooks_obj[hook_name] = Value::Array(merged);
    }

    write_json(path, &settings);
}

fn uninstall_hooks_from_file(path: &PathBuf, hook_defs: &Value) {
    if !path.exists() {
        return;
    }

    let mut settings = read_json(path);

    if let Some(hooks_obj) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let hook_names: Vec<String> = hook_defs
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();

        for hook_name in &hook_names {
            if let Some(existing) = hooks_obj.get(hook_name).and_then(|v| v.as_array()) {
                let non_shelly: Vec<Value> = existing
                    .iter()
                    .filter(|e| !is_shelly_hook(e))
                    .cloned()
                    .collect();

                if non_shelly.is_empty() {
                    hooks_obj.remove(hook_name);
                } else {
                    hooks_obj.insert(hook_name.clone(), Value::Array(non_shelly));
                }
            }
        }

        if hooks_obj.is_empty() {
            settings.as_object_mut().unwrap().remove("hooks");
        }
    }

    write_json(path, &settings);
}

// --- Bridge script installation ---

fn install_bridge_script() {
    let dest = bridge_script_path();
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let script = include_str!("../resources/shelly-bridge.py");
    let _ = fs::write(&dest, script);

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(0o755));
    }

    log::info!("Bridge script installed to {}", dest.display());
}

fn uninstall_bridge_script() {
    let dest = bridge_script_path();
    if dest.exists() {
        let _ = fs::remove_file(&dest);
    }
    // Clean up ~/.shelly if empty
    let shelly_dir = home_dir().join(".shelly");
    if shelly_dir.exists() {
        let _ = fs::remove_dir(&shelly_dir); // only removes if empty
    }
}

// --- Public API ---

pub fn install() {
    // Install bridge script for Codex/Gemini
    install_bridge_script();

    // Claude Code (HTTP hooks)
    install_hooks_to_file(&claude_settings_path(), &claude_hooks());
    log::info!("Claude Code hooks installed");

    // Codex CLI (command hooks)
    install_hooks_to_file(&codex_hooks_path(), &codex_hooks());
    log::info!("Codex CLI hooks installed");

    // Gemini CLI (command hooks)
    install_hooks_to_file(&gemini_settings_path(), &gemini_hooks());
    log::info!("Gemini CLI hooks installed");
}

pub fn uninstall() {
    // Claude Code
    uninstall_hooks_from_file(&claude_settings_path(), &claude_hooks());
    log::info!("Claude Code hooks removed");

    // Codex CLI
    uninstall_hooks_from_file(&codex_hooks_path(), &codex_hooks());
    log::info!("Codex CLI hooks removed");

    // Gemini CLI
    uninstall_hooks_from_file(&gemini_settings_path(), &gemini_hooks());
    log::info!("Gemini CLI hooks removed");

    // Remove bridge script
    uninstall_bridge_script();
}

/// Add a PreToolUse allow rule so the tool is auto-approved from now on.
pub fn add_allow_rule(tool_name: &str) {
    let mut settings = read_json(&claude_settings_path());
    let hooks_obj = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));

    let pre_tool_use = hooks_obj
        .as_object_mut()
        .unwrap()
        .entry("PreToolUse")
        .or_insert_with(|| Value::Array(Vec::new()));

    let entries = pre_tool_use.as_array_mut().unwrap();

    // Check if there's already a Shelly allow rule for this tool
    let already_exists = entries.iter().any(|e| {
        is_shelly_hook(e)
            && e.get("matcher")
                .and_then(|m| m.as_str())
                .map(|m| m == tool_name)
                .unwrap_or(false)
    });

    if !already_exists {
        // Add a PreToolUse hook that auto-allows this tool
        entries.push(serde_json::json!({
            "matcher": tool_name,
            "hooks": [{
                "type": "http",
                "url": format!("http://{}/hooks/auto-allow", SHELLY_MARKER),
                "timeout": 5
            }]
        }));
    }

    write_json(&claude_settings_path(), &settings);
    log::info!("Added allow-always rule for tool: {}", tool_name);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- is_shelly_hook ---

    #[test]
    fn detects_http_shelly_hook() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{"type": "http", "url": "http://localhost:21517/hooks/permission"}]
        });
        assert!(is_shelly_hook(&entry));
    }

    #[test]
    fn detects_command_shelly_hook() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{"type": "command", "command": "python3 /Users/x/.shelly/shelly-bridge.py codex-cli permission"}]
        });
        assert!(is_shelly_hook(&entry));
    }

    #[test]
    fn rejects_non_shelly_http_hook() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{"type": "http", "url": "http://localhost:9999/other"}]
        });
        assert!(!is_shelly_hook(&entry));
    }

    #[test]
    fn rejects_non_shelly_command_hook() {
        let entry = serde_json::json!({
            "matcher": "",
            "hooks": [{"type": "command", "command": "my-custom-script.sh"}]
        });
        assert!(!is_shelly_hook(&entry));
    }

    #[test]
    fn rejects_empty_hooks_array() {
        let entry = serde_json::json!({"matcher": "", "hooks": []});
        assert!(!is_shelly_hook(&entry));
    }

    #[test]
    fn rejects_missing_hooks_key() {
        let entry = serde_json::json!({"matcher": ""});
        assert!(!is_shelly_hook(&entry));
    }

    // --- hook definitions ---

    #[test]
    fn claude_hooks_has_expected_events() {
        let hooks = claude_hooks();
        let obj = hooks.as_object().unwrap();
        assert!(obj.contains_key("PreToolUse"));
        assert!(obj.contains_key("Notification"));
        assert!(obj.contains_key("PermissionRequest"));
        assert!(obj.contains_key("Stop"));
    }

    #[test]
    fn codex_hooks_has_expected_events() {
        let hooks = codex_hooks();
        let obj = hooks.as_object().unwrap();
        assert!(obj.contains_key("PreToolUse"));
        assert!(obj.contains_key("Stop"));
        assert!(!obj.contains_key("PermissionRequest")); // Codex uses PreToolUse for permissions
    }

    #[test]
    fn gemini_hooks_has_expected_events() {
        let hooks = gemini_hooks();
        let obj = hooks.as_object().unwrap();
        assert!(obj.contains_key("BeforeTool"));
        assert!(obj.contains_key("Notification"));
        assert!(obj.contains_key("SessionEnd"));
    }

    #[test]
    fn codex_hooks_use_command_type() {
        let hooks = codex_hooks();
        let pre = &hooks["PreToolUse"][0]["hooks"][0];
        assert_eq!(pre["type"], "command");
        assert!(pre["command"].as_str().unwrap().contains("shelly-bridge.py"));
        assert!(pre["command"].as_str().unwrap().contains("codex-cli"));
    }

    #[test]
    fn gemini_hooks_use_command_type() {
        let hooks = gemini_hooks();
        let before_tool = &hooks["BeforeTool"][0]["hooks"][0];
        assert_eq!(before_tool["type"], "command");
        assert!(before_tool["command"].as_str().unwrap().contains("shelly-bridge.py"));
        assert!(before_tool["command"].as_str().unwrap().contains("gemini-cli"));
    }

    // --- install_hooks_to_file / uninstall_hooks_from_file ---

    #[test]
    fn install_to_empty_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let hooks = serde_json::json!({
            "MyEvent": [{"matcher": "", "hooks": [{"type": "http", "url": "http://localhost:21517/test"}]}]
        });

        install_hooks_to_file(&path, &hooks);

        let result = read_json(&path);
        assert_eq!(result["hooks"]["MyEvent"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn install_preserves_existing_non_shelly_hooks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        // Write a pre-existing hook
        let existing = serde_json::json!({
            "hooks": {
                "PreToolUse": [{"matcher": "lint", "hooks": [{"type": "command", "command": "my-linter"}]}]
            }
        });
        write_json(&path, &existing);

        // Install Shelly hooks
        install_hooks_to_file(&path, &codex_hooks());

        let result = read_json(&path);
        let pre_tool = result["hooks"]["PreToolUse"].as_array().unwrap();
        // Should have the user's hook + Shelly's hook
        assert_eq!(pre_tool.len(), 2);
        assert_eq!(pre_tool[0]["hooks"][0]["command"], "my-linter");
        assert!(pre_tool[1]["hooks"][0]["command"].as_str().unwrap().contains("shelly-bridge.py"));
    }

    #[test]
    fn install_replaces_stale_shelly_hooks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        // Install once
        install_hooks_to_file(&path, &codex_hooks());
        // Install again (should not duplicate)
        install_hooks_to_file(&path, &codex_hooks());

        let result = read_json(&path);
        let pre_tool = result["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
    }

    #[test]
    fn uninstall_removes_shelly_hooks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        install_hooks_to_file(&path, &codex_hooks());
        uninstall_hooks_from_file(&path, &codex_hooks());

        let result = read_json(&path);
        // hooks key should be removed entirely when empty
        assert!(result.get("hooks").is_none());
    }

    #[test]
    fn uninstall_preserves_non_shelly_hooks() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let existing = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"matcher": "lint", "hooks": [{"type": "command", "command": "my-linter"}]},
                    {"matcher": "", "hooks": [{"type": "command", "command": "python3 /x/shelly-bridge.py codex-cli permission"}]}
                ]
            }
        });
        write_json(&path, &existing);

        uninstall_hooks_from_file(&path, &codex_hooks());

        let result = read_json(&path);
        let pre_tool = result["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre_tool.len(), 1);
        assert_eq!(pre_tool[0]["hooks"][0]["command"], "my-linter");
    }

    #[test]
    fn uninstall_noop_on_missing_file() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("nonexistent.json");

        // Should not panic
        uninstall_hooks_from_file(&path, &codex_hooks());
        assert!(!path.exists());
    }

    #[test]
    fn install_preserves_non_hook_settings() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");

        let existing = serde_json::json!({
            "model": "o4-mini",
            "approvalMode": "suggest"
        });
        write_json(&path, &existing);

        install_hooks_to_file(&path, &codex_hooks());

        let result = read_json(&path);
        assert_eq!(result["model"], "o4-mini");
        assert_eq!(result["approvalMode"], "suggest");
        assert!(result["hooks"]["PreToolUse"].as_array().unwrap().len() > 0);
    }
}
