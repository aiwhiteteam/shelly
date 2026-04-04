use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const SHELLY_MARKER: &str = "localhost:21517";

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("settings.json")
}

fn read_settings() -> Value {
    let path = settings_path();
    if !path.exists() {
        return Value::Object(Default::default());
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Object(Default::default()))
}

fn write_settings(settings: &Value) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(settings).unwrap_or_default();
    let _ = fs::write(&path, format!("{}\n", json));
}

fn is_shelly_hook(entry: &Value) -> bool {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks.iter().any(|h| {
                h.get("url")
                    .and_then(|u| u.as_str())
                    .map(|u| u.contains(SHELLY_MARKER))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn shelly_hooks() -> Value {
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

pub fn install() {
    let mut settings = read_settings();
    let hooks_obj = settings
        .as_object_mut()
        .unwrap()
        .entry("hooks")
        .or_insert_with(|| Value::Object(Default::default()));

    let shelly = shelly_hooks();
    for (hook_name, shelly_entries) in shelly.as_object().unwrap() {
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

    write_settings(&settings);
    log::info!("Shelly hooks installed");
}

pub fn uninstall() {
    let mut settings = read_settings();

    if let Some(hooks_obj) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        let hook_names: Vec<String> = shelly_hooks()
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

    write_settings(&settings);
    log::info!("Shelly hooks removed");
}

/// Add a PreToolUse allow rule so the tool is auto-approved from now on.
pub fn add_allow_rule(tool_name: &str) {
    let mut settings = read_settings();
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

    write_settings(&settings);
    log::info!("Added allow-always rule for tool: {}", tool_name);
}
