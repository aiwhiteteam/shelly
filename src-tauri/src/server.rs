use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::oneshot;

const PORT: u16 = 21517;

// --- Types ---

#[derive(Clone)]
struct AppState {
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    pending_questions: Arc<Mutex<HashMap<String, oneshot::Sender<PreToolUseDecision>>>>,
    tauri_handle: tauri::AppHandle,
    yolo_mode: Arc<AtomicBool>,
}

#[derive(Serialize, Deserialize, Clone)]
struct PermissionDecision {
    behavior: String,
}

#[derive(Serialize, Deserialize, Clone)]
struct PreToolUseDecision {
    permission_decision: String,
    permission_decision_reason: Option<String>,
    updated_input: Option<Value>,
}

// --- Events emitted to frontend ---

#[derive(Serialize, Clone)]
struct NotificationPayload {
    session_id: String,
    agent: String,
    message: String,
    level: String,
}

#[derive(Serialize, Clone)]
struct PermissionPayload {
    request_id: String,
    session_id: String,
    agent: String,
    tool_name: String,
    tool_input: Value,
    project: Option<String>,
}

#[derive(Serialize, Clone)]
struct QuestionPayload {
    request_id: String,
    session_id: String,
    agent: String,
    tool_name: String,
    tool_input: Value,
    project: Option<String>,
}

#[derive(Serialize, Clone)]
struct StopPayload {
    session_id: String,
    agent: String,
    reason: Option<String>,
    duration_ms: Option<u64>,
}

// --- Global state for resolving from Tauri commands ---

static GLOBAL_STATE: std::sync::OnceLock<AppState> = std::sync::OnceLock::new();

pub fn resolve_permission(request_id: &str, behavior: &str) {
    if let Some(state) = GLOBAL_STATE.get() {
        if let Some(tx) = state.pending_permissions.lock().unwrap().remove(request_id) {
            let _ = tx.send(PermissionDecision {
                behavior: behavior.to_string(),
            });
        }
    }
}

pub fn set_yolo_mode(enabled: bool) {
    if let Some(state) = GLOBAL_STATE.get() {
        state.yolo_mode.store(enabled, Ordering::Relaxed);
        log::info!("Yolo mode set to {}", enabled);
    }
}

pub fn resolve_pre_tool_use(request_id: &str, permission_decision: &str, updated_input: Option<Value>) {
    if let Some(state) = GLOBAL_STATE.get() {
        if let Some(tx) = state.pending_questions.lock().unwrap().remove(request_id) {
            let _ = tx.send(PreToolUseDecision {
                permission_decision: permission_decision.to_string(),
                permission_decision_reason: Some("Answered via Shelly".to_string()),
                updated_input,
            });
        }
    }
}

fn detect_agent(body: &Value) -> String {
    body.get("agent")
        .and_then(|v| v.as_str())
        .filter(|a| ["claude-code", "codex-cli", "gemini-cli", "cursor", "opencode"].contains(a))
        .unwrap_or("claude-code")
        .to_string()
}

/// Look up the project folder name for a session by checking the hook body's cwd
/// or scanning ~/.claude/sessions/ for a matching sessionId.
fn lookup_project(body: &Value, session_id: &str) -> Option<String> {
    // First check if cwd is in the hook body
    if let Some(cwd) = body.get("cwd").and_then(|v| v.as_str()) {
        return std::path::Path::new(cwd)
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
    }

    // Fall back to scanning session files
    let sessions_dir = dirs::home_dir()?.join(".claude").join("sessions");
    if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(data) = serde_json::from_str::<Value>(&content) {
                        let sid = data.get("sessionId").and_then(|v| v.as_str()).unwrap_or("");
                        let pid = data.get("pid").and_then(|v| v.as_u64()).map(|p| p.to_string()).unwrap_or_default();
                        if sid == session_id || pid == session_id {
                            if let Some(cwd) = data.get("cwd").and_then(|v| v.as_str()) {
                                return std::path::Path::new(cwd)
                                    .file_name()
                                    .map(|n| n.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn gen_id(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string())
}

/// Helper: dismiss a pending request from both backend state and frontend UI.
fn dismiss(state: &AppState, request_id: &str, kind: &str) {
    match kind {
        "permission" => { state.pending_permissions.lock().unwrap().remove(request_id); }
        "question" => { state.pending_questions.lock().unwrap().remove(request_id); }
        _ => {}
    }
    let _ = state.tauri_handle.emit("shelly://dismiss", serde_json::json!({
        "request_id": request_id,
    }));
    log::info!("Dismissed {} {}", kind, request_id);
}

// --- Handlers ---

async fn health(State(state): State<AppState>) -> Json<Value> {
    let perms = state.pending_permissions.lock().unwrap().len();
    let questions = state.pending_questions.lock().unwrap().len();
    Json(serde_json::json!({
        "status": "ok",
        "pendingPermissions": perms,
        "pendingQuestions": questions,
        "version": "1.0.0"
    }))
}

async fn notification(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let payload = NotificationPayload {
        session_id: body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        agent: detect_agent(&body),
        message: body.get("message").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        level: body.get("level").and_then(|v| v.as_str()).unwrap_or("info").to_string(),
    };
    let _ = state.tauri_handle.emit("shelly://notification", &payload);
    Json(serde_json::json!({"status": "ok"}))
}

async fn pre_tool_use(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let request_id = gen_id("ptu");
    let session_id = body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let project = lookup_project(&body, &session_id);
    let payload = QuestionPayload {
        request_id: request_id.clone(),
        session_id,
        agent: detect_agent(&body),
        tool_name: body.get("tool_name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        tool_input: body.get("tool_input").cloned().unwrap_or(Value::Object(Default::default())),
        project,
    };

    let (tx, rx) = oneshot::channel::<PreToolUseDecision>();
    state.pending_questions.lock().unwrap().insert(request_id.clone(), tx);

    let _ = state.tauri_handle.emit("shelly://question", &payload);

    // Block until user answers or timeout
    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => {
            let mut hook_output = serde_json::json!({
                "hookEventName": "PreToolUse",
                "permissionDecision": decision.permission_decision,
            });
            if let Some(reason) = &decision.permission_decision_reason {
                hook_output["permissionDecisionReason"] = Value::String(reason.clone());
            }
            if let Some(input) = &decision.updated_input {
                hook_output["updatedInput"] = input.clone();
            }
            Json(serde_json::json!({ "hookSpecificOutput": hook_output }))
        }
        _ => {
            dismiss(&state, &request_id, "question");
            Json(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow"
                }
            }))
        }
    }
}

fn permission_allow_response() -> Value {
    serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PermissionRequest",
            "decision": { "behavior": "allow" }
        }
    })
}

/// Returns Some(allow) if the permission should be auto-approved (AskUserQuestion bypass or yolo mode).
/// Returns None if the permission needs interactive approval.
fn check_auto_approve(tool_name: &str, yolo_mode: bool) -> Option<Value> {
    if tool_name == "AskUserQuestion" || tool_name == "ask_user_question" {
        return Some(permission_allow_response());
    }
    if yolo_mode {
        return Some(permission_allow_response());
    }
    None
}

async fn permission(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let tool_name = body.get("tool_name").or(body.get("tool")).and_then(|v| v.as_str()).unwrap_or("");

    if let Some(response) = check_auto_approve(tool_name, state.yolo_mode.load(Ordering::Relaxed)) {
        if tool_name != "AskUserQuestion" && tool_name != "ask_user_question" {
            log::info!("Yolo mode: auto-approving permission for {}", tool_name);
        }
        return Json(response);
    }

    log::info!("Permission request received: {}", serde_json::to_string(&body).unwrap_or_default());
    let request_id = gen_id("perm");
    let session_id = body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let project = lookup_project(&body, &session_id);
    let payload = PermissionPayload {
        request_id: request_id.clone(),
        session_id,
        agent: detect_agent(&body),
        tool_name: body.get("tool_name").or(body.get("tool")).and_then(|v| v.as_str()).unwrap_or("Unknown Tool").to_string(),
        tool_input: body.get("tool_input").or(body.get("input")).cloned().unwrap_or(Value::Object(Default::default())),
        project,
    };
    log::info!("Emitting shelly://permission with request_id: {}", request_id);

    let (tx, rx) = oneshot::channel::<PermissionDecision>();
    state.pending_permissions.lock().unwrap().insert(request_id.clone(), tx);

    let _ = state.tauri_handle.emit("shelly://permission", &payload);

    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => {
            Json(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": {
                        "behavior": decision.behavior
                    }
                }
            }))
        }
        _ => {
            dismiss(&state, &request_id, "permission");
            Json(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PermissionRequest",
                    "decision": { "behavior": "deny" }
                }
            }))
        }
    }
}

async fn auto_allow() -> Json<Value> {
    Json(serde_json::json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "allow",
            "permissionDecisionReason": "Auto-allowed by Shelly"
        }
    }))
}

async fn stop(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let payload = StopPayload {
        session_id: body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        agent: detect_agent(&body),
        reason: body.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
        duration_ms: body.get("duration_ms").and_then(|v| v.as_u64()),
    };
    let _ = state.tauri_handle.emit("shelly://stop", &payload);
    Json(serde_json::json!({"status": "ok"}))
}

// --- Stale request cleanup ---

/// Background task: periodically checks for stale pending requests where the
/// oneshot sender's receiver has been dropped (client disconnected). Dismisses
/// them from the frontend queue.
async fn cleanup_stale_requests(state: AppState) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Check permissions: if tx.is_closed(), the handler future was dropped (client disconnect)
        let stale_perms: Vec<String> = {
            let map = state.pending_permissions.lock().unwrap();
            map.iter()
                .filter(|(_, tx)| tx.is_closed())
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in stale_perms {
            dismiss(&state, &id, "permission");
            log::info!("Cleaned up stale permission request {}", id);
        }

        // Check questions
        let stale_questions: Vec<String> = {
            let map = state.pending_questions.lock().unwrap();
            map.iter()
                .filter(|(_, tx)| tx.is_closed())
                .map(|(id, _)| id.clone())
                .collect()
        };
        for id in stale_questions {
            dismiss(&state, &id, "question");
            log::info!("Cleaned up stale question request {}", id);
        }
    }
}

// --- Start server ---

pub async fn start(handle: tauri::AppHandle) {
    let state = AppState {
        pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        pending_questions: Arc::new(Mutex::new(HashMap::new())),
        tauri_handle: handle,
        yolo_mode: Arc::new(AtomicBool::new(false)),
    };

    let _ = GLOBAL_STATE.set(state.clone());

    // Spawn background cleanup for stale requests (answered in terminal)
    tokio::spawn(cleanup_stale_requests(state.clone()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/hooks/notification", post(notification))
        .route("/hooks/pre-tool-use", post(pre_tool_use))
        .route("/hooks/permission", post(permission))
        .route("/hooks/auto-allow", post(auto_allow))
        .route("/hooks/stop", post(stop))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:21517")
        .await
        .expect("Failed to bind port 21517");

    log::info!("Shelly server listening on http://127.0.0.1:{}", PORT);

    axum::serve(listener, app).await.unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- detect_agent ---

    #[test]
    fn detect_agent_known_agents() {
        for agent in &["claude-code", "codex-cli", "gemini-cli", "cursor", "opencode"] {
            let body = serde_json::json!({ "agent": agent });
            assert_eq!(detect_agent(&body), *agent);
        }
    }

    #[test]
    fn detect_agent_unknown_defaults_to_claude_code() {
        let body = serde_json::json!({ "agent": "unknown-agent" });
        assert_eq!(detect_agent(&body), "claude-code");
    }

    #[test]
    fn detect_agent_missing_defaults_to_claude_code() {
        let body = serde_json::json!({});
        assert_eq!(detect_agent(&body), "claude-code");
    }

    #[test]
    fn detect_agent_non_string_defaults_to_claude_code() {
        let body = serde_json::json!({ "agent": 42 });
        assert_eq!(detect_agent(&body), "claude-code");
    }

    // --- gen_id ---

    #[test]
    fn gen_id_has_correct_prefix() {
        let id = gen_id("perm");
        assert!(id.starts_with("perm_"), "Expected prefix 'perm_', got: {}", id);
    }

    #[test]
    fn gen_id_has_correct_length() {
        let id = gen_id("ptu");
        // "ptu_" (4) + 12 hex chars = 16
        assert_eq!(id.len(), 16, "Expected length 16, got: {} ({})", id.len(), id);
    }

    #[test]
    fn gen_id_produces_unique_ids() {
        let id1 = gen_id("test");
        let id2 = gen_id("test");
        assert_ne!(id1, id2);
    }

    // --- check_auto_approve ---

    #[test]
    fn auto_approve_ask_user_question_always_allowed() {
        let result = check_auto_approve("AskUserQuestion", false);
        assert!(result.is_some());
        let decision = &result.unwrap()["hookSpecificOutput"]["decision"]["behavior"];
        assert_eq!(decision, "allow");
    }

    #[test]
    fn auto_approve_ask_user_question_snake_case() {
        let result = check_auto_approve("ask_user_question", false);
        assert!(result.is_some());
        let decision = &result.unwrap()["hookSpecificOutput"]["decision"]["behavior"];
        assert_eq!(decision, "allow");
    }

    #[test]
    fn auto_approve_yolo_mode_approves_any_tool() {
        for tool in &["Bash", "Write", "Edit", "Read", "SomeCustomTool"] {
            let result = check_auto_approve(tool, true);
            assert!(result.is_some(), "Yolo mode should auto-approve {}", tool);
            let decision = &result.unwrap()["hookSpecificOutput"]["decision"]["behavior"];
            assert_eq!(decision, "allow");
        }
    }

    #[test]
    fn auto_approve_normal_mode_requires_interaction() {
        for tool in &["Bash", "Write", "Edit", "Read", "SomeCustomTool"] {
            let result = check_auto_approve(tool, false);
            assert!(result.is_none(), "Normal mode should NOT auto-approve {}", tool);
        }
    }

    #[test]
    fn auto_approve_empty_tool_name_normal_mode() {
        let result = check_auto_approve("", false);
        assert!(result.is_none());
    }

    #[test]
    fn auto_approve_empty_tool_name_yolo_mode() {
        let result = check_auto_approve("", true);
        assert!(result.is_some());
    }

    // --- permission_allow_response format ---

    #[test]
    fn permission_allow_response_has_correct_structure() {
        let resp = permission_allow_response();
        assert_eq!(resp["hookSpecificOutput"]["hookEventName"], "PermissionRequest");
        assert_eq!(resp["hookSpecificOutput"]["decision"]["behavior"], "allow");
    }

    // --- yolo_mode AtomicBool ---

    #[test]
    fn yolo_mode_atomic_toggle() {
        let flag = Arc::new(AtomicBool::new(false));
        assert!(!flag.load(Ordering::Relaxed));

        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));

        flag.store(false, Ordering::Relaxed);
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn yolo_mode_shared_across_clones() {
        let flag = Arc::new(AtomicBool::new(false));
        let flag2 = flag.clone();

        flag.store(true, Ordering::Relaxed);
        assert!(flag2.load(Ordering::Relaxed));

        flag2.store(false, Ordering::Relaxed);
        assert!(!flag.load(Ordering::Relaxed));
    }

    // --- lookup_project ---

    #[test]
    fn lookup_project_extracts_from_cwd_in_body() {
        let body = serde_json::json!({"cwd": "/Users/foo/projects/my-app"});
        assert_eq!(lookup_project(&body, "any"), Some("my-app".to_string()));
    }

    #[test]
    fn lookup_project_returns_none_without_cwd() {
        let body = serde_json::json!({"session_id": "test"});
        // Will try to scan ~/.claude/sessions/ which may or may not have matches
        // but with a fake session_id it should return None
        let result = lookup_project(&body, "nonexistent-session-id-12345");
        assert!(result.is_none());
    }

    #[test]
    fn lookup_project_handles_root_cwd() {
        let body = serde_json::json!({"cwd": "/"});
        // Root path has no file_name
        let result = lookup_project(&body, "any");
        assert!(result.is_none());
    }
}
