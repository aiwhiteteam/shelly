use axum::{extract::State, routing::{get, post}, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::Emitter;
use tokio::sync::oneshot;

const PORT: u16 = 21517;

// --- Drop guard: detects client disconnect and dismisses the UI ---

struct PendingGuard {
    request_id: String,
    kind: &'static str, // "permission" or "question"
    state: AppState,
    defused: bool,
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        if self.defused {
            return;
        }
        // Client disconnected or handler was cancelled — clean up and notify frontend
        match self.kind {
            "permission" => { self.state.pending_permissions.lock().unwrap().remove(&self.request_id); }
            "question" => { self.state.pending_questions.lock().unwrap().remove(&self.request_id); }
            _ => {}
        }
        let _ = self.state.tauri_handle.emit("shelly://dismiss", serde_json::json!({
            "request_id": self.request_id,
        }));
        log::info!("Client disconnected for {} {}, dismissed UI", self.kind, self.request_id);
    }
}

// --- Types ---

#[derive(Clone)]
struct AppState {
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
    pending_questions: Arc<Mutex<HashMap<String, oneshot::Sender<PreToolUseDecision>>>>,
    tauri_handle: tauri::AppHandle,
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
}

#[derive(Serialize, Clone)]
struct QuestionPayload {
    request_id: String,
    session_id: String,
    agent: String,
    tool_name: String,
    tool_input: Value,
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

fn gen_id(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4().to_string().replace("-", "")[..12].to_string())
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
    let payload = QuestionPayload {
        request_id: request_id.clone(),
        session_id: body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        agent: detect_agent(&body),
        tool_name: body.get("tool_name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
        tool_input: body.get("tool_input").cloned().unwrap_or(Value::Object(Default::default())),
    };

    let (tx, rx) = oneshot::channel::<PreToolUseDecision>();
    state.pending_questions.lock().unwrap().insert(request_id.clone(), tx);

    let mut guard = PendingGuard {
        request_id: request_id.clone(),
        kind: "question",
        state: state.clone(),
        defused: false,
    };

    let _ = state.tauri_handle.emit("shelly://question", &payload);

    // Block until user answers or timeout
    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => {
            guard.defused = true;
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
            guard.defused = true;
            state.pending_questions.lock().unwrap().remove(&request_id);
            let _ = state.tauri_handle.emit("shelly://dismiss", serde_json::json!({
                "request_id": request_id,
            }));
            // Timeout or error — allow passthrough
            Json(serde_json::json!({
                "hookSpecificOutput": {
                    "hookEventName": "PreToolUse",
                    "permissionDecision": "allow"
                }
            }))
        }
    }
}

async fn permission(State(state): State<AppState>, Json(body): Json<Value>) -> Json<Value> {
    let tool_name = body.get("tool_name").or(body.get("tool")).and_then(|v| v.as_str()).unwrap_or("");

    // AskUserQuestion is already handled by PreToolUse hook — auto-allow here
    if tool_name == "AskUserQuestion" || tool_name == "ask_user_question" {
        return Json(serde_json::json!({
            "hookSpecificOutput": {
                "hookEventName": "PermissionRequest",
                "decision": { "behavior": "allow" }
            }
        }));
    }

    log::info!("Permission request received: {}", serde_json::to_string(&body).unwrap_or_default());
    let request_id = gen_id("perm");
    let payload = PermissionPayload {
        request_id: request_id.clone(),
        session_id: body.get("session_id").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
        agent: detect_agent(&body),
        tool_name: body.get("tool_name").or(body.get("tool")).and_then(|v| v.as_str()).unwrap_or("Unknown Tool").to_string(),
        tool_input: body.get("tool_input").or(body.get("input")).cloned().unwrap_or(Value::Object(Default::default())),
    };
    log::info!("Emitting shelly://permission with request_id: {}", request_id);

    let (tx, rx) = oneshot::channel::<PermissionDecision>();
    state.pending_permissions.lock().unwrap().insert(request_id.clone(), tx);

    let mut guard = PendingGuard {
        request_id: request_id.clone(),
        kind: "permission",
        state: state.clone(),
        defused: false,
    };

    let _ = state.tauri_handle.emit("shelly://permission", &payload);

    match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => {
            guard.defused = true;
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
            guard.defused = true;
            state.pending_permissions.lock().unwrap().remove(&request_id);
            let _ = state.tauri_handle.emit("shelly://dismiss", serde_json::json!({
                "request_id": request_id,
            }));
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

// --- Start server ---

pub async fn start(handle: tauri::AppHandle) {
    let state = AppState {
        pending_permissions: Arc::new(Mutex::new(HashMap::new())),
        pending_questions: Arc::new(Mutex::new(HashMap::new())),
        tauri_handle: handle,
    };

    let _ = GLOBAL_STATE.set(state.clone());

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
