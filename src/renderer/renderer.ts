// Agent config
const AGENT_CONFIGS: Record<string, { name: string; color: string; icon: string }> = {
  "claude-code": { name: "Claude Code", color: "#d97757", icon: "⚡" },
  "codex-cli": { name: "Codex CLI", color: "#22c55e", icon: "◆" },
  "gemini-cli": { name: "Gemini CLI", color: "#3b82f6", icon: "✦" },
  cursor: { name: "Cursor", color: "#a855f7", icon: "▸" },
  opencode: { name: "OpenCode", color: "#f59e0b", icon: "○" },
};

// Tauri API
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

// ─── DOM Elements ────────────────────────────────────────────────────

const container = document.getElementById("container")!;
const idleView = document.getElementById("idle-view")!;
const sessionInfo = document.getElementById("session-info")!;
const notificationView = document.getElementById("notification-view")!;
const notificationBadge = document.getElementById("notification-badge")!;
const notificationIcon = document.getElementById("notification-icon")!;
const notificationMessage = document.getElementById("notification-message")!;
const permissionView = document.getElementById("permission-view")!;
const permissionIcon = document.getElementById("permission-icon")!;
const permissionTool = document.getElementById("permission-tool")!;
const permissionDetail = document.getElementById("permission-detail")!;
const questionView = document.getElementById("question-view")!;
const questionText = document.getElementById("question-text")!;
const questionOptions = document.getElementById("question-options")!;
const questionInputRow = document.getElementById("question-input-row")!;
const questionInput = document.getElementById("question-input")! as HTMLInputElement;
const questionInputSubmit = document.getElementById("question-input-submit")!;
const stopView = document.getElementById("stop-view")!;
const stopIcon = document.getElementById("stop-icon")!;
const stopMessage = document.getElementById("stop-message")!;
const btnAllow = document.getElementById("btn-allow")! as HTMLButtonElement;
const btnAllowAlways = document.getElementById("btn-allow-always")! as HTMLButtonElement;
const btnDeny = document.getElementById("btn-deny")! as HTMLButtonElement;

const permissionProject = document.getElementById("permission-project")!;
const questionProject = document.getElementById("question-project")!;
const btnJumpPerm = document.getElementById("btn-jump-perm")! as HTMLButtonElement;
const btnJumpQuestion = document.getElementById("btn-jump-question")! as HTMLButtonElement;
const btnJumpStop = document.getElementById("btn-jump-stop")! as HTMLButtonElement;
const btnJumpNotif = document.getElementById("btn-jump-notif")! as HTMLButtonElement;
const btnMute = document.getElementById("btn-mute")! as HTMLButtonElement;
const muteIcon = document.getElementById("mute-icon")!;
const btnTheme = document.getElementById("btn-theme")! as HTMLButtonElement;
const themeIcon = document.getElementById("theme-icon")!;
const btnYolo = document.getElementById("btn-yolo")! as HTMLButtonElement;
const yoloIcon = document.getElementById("yolo-icon")!;
const btnMinimize = document.getElementById("btn-minimize")! as HTMLButtonElement;
const ghostIcon = document.getElementById("ghost-icon")!;
const btnClose = document.getElementById("btn-close")! as HTMLButtonElement;
const pendingBadge = document.getElementById("pending-badge")!;
const ghostFeedback = document.getElementById("ghost-feedback")!;
const ghostFeedbackIcon = document.getElementById("ghost-feedback-icon")!;
const ghostFeedbackText = document.getElementById("ghost-feedback-text")!;

// State
let currentPermissionRequestId: string | null = null;
let currentPermissionToolName: string | null = null;
let currentPermissionPayload: any = null;
let currentQuestionRequestId: string | null = null;
let currentQuestionToolInput: Record<string, unknown> | null = null;
let currentQuestionPayload: any = null;
let currentSessionId: string | null = null;
let hideTimer: ReturnType<typeof setTimeout> | null = null;
let showTime: number | null = null;
let elapsedInterval: ReturnType<typeof setInterval> | null = null;

// ─── Event Queue ─────────────────────────────────────────────────────
type QueuedEvent =
  | { type: "permission"; payload: any }
  | { type: "question"; payload: any }
  | { type: "notification"; payload: any }
  | { type: "stop"; payload: any };

const eventQueue: QueuedEvent[] = [];
let isProcessingEvent = false;

function hasPendingInteractive(): boolean {
  return eventQueue.some(e => e.type === "permission" || e.type === "question");
}

function updatePendingBadge() {
  const queued = eventQueue.filter(e => e.type === "permission" || e.type === "question").length;
  const showing = (currentPermissionRequestId || currentQuestionRequestId) ? 1 : 0;
  const total = queued + showing;
  if (total > 1) {
    pendingBadge.textContent = `${total} pending`;
    pendingBadge.classList.remove("hidden");
  } else {
    pendingBadge.classList.add("hidden");
  }
}

function processNextEvent() {
  if (eventQueue.length === 0) {
    isProcessingEvent = false;
    updatePendingBadge();
    if (ghostMode) {
      ghostHide();
    } else {
      showIdleView();
    }
    return;
  }

  isProcessingEvent = true;
  const next = eventQueue.shift()!;
  updatePendingBadge();

  if (next.type === "permission") {
    showPermissionEvent(next.payload);
  } else if (next.type === "question") {
    showQuestionEvent(next.payload);
  } else if (next.type === "notification") {
    showNotificationEvent(next.payload);
  } else if (next.type === "stop") {
    showStopEvent(next.payload);
  }
}

function enqueueEvent(event: QueuedEvent) {
  // Notifications and stop events can show immediately if nothing is active,
  // otherwise they queue like everything else
  if (event.type === "notification" || event.type === "stop") {
    if (!isProcessingEvent) {
      isProcessingEvent = true;
      expand();
      if (event.type === "notification") {
        showNotificationEvent(event.payload);
      } else {
        showStopEvent(event.payload);
      }
    } else {
      eventQueue.push(event);
      updatePendingBadge();
    }
    return;
  }

  // Permission/Question: queue if something is already showing
  if (isProcessingEvent) {
    eventQueue.push(event);
    updatePendingBadge();
    return;
  }

  isProcessingEvent = true;
  expand();
  if (event.type === "permission") {
    showPermissionEvent(event.payload);
  } else {
    showQuestionEvent(event.payload);
  }
  updatePendingBadge();
}
let isMuted = localStorage.getItem("shelly-muted") === "true";
let ghostMode = false; // ghost mode off by default
let yoloMode = false; // yolo mode (auto-approve) off by default

// Theme: "glass" (default) | "white" | "dark"
const THEMES = ["glass", "white", "dark"] as const;
type Theme = typeof THEMES[number];
let currentTheme: Theme = (localStorage.getItem("shelly-theme") as Theme) || "dark";

// ─── 8-bit Sound Synthesis ───────────────────────────────────────────

let audioCtx: AudioContext | null = null;

function getAudioCtx(): AudioContext {
  if (!audioCtx) audioCtx = new AudioContext();
  return audioCtx;
}

function playSound(type: "notification" | "permission" | "question" | "stop" | "allow" | "deny") {
  if (isMuted) return;
  try {
    const ctx = getAudioCtx();
    const now = ctx.currentTime;

    switch (type) {
      case "notification": {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "square";
        osc.frequency.setValueAtTime(440, now);
        osc.frequency.setValueAtTime(660, now + 0.06);
        gain.gain.setValueAtTime(0.08, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.15);
        osc.connect(gain).connect(ctx.destination);
        osc.start(now);
        osc.stop(now + 0.15);
        break;
      }
      case "permission": {
        for (let i = 0; i < 3; i++) {
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();
          osc.type = "square";
          osc.frequency.setValueAtTime(330 + i * 110, now + i * 0.1);
          gain.gain.setValueAtTime(0.07, now + i * 0.1);
          gain.gain.exponentialRampToValueAtTime(0.001, now + i * 0.1 + 0.08);
          osc.connect(gain).connect(ctx.destination);
          osc.start(now + i * 0.1);
          osc.stop(now + i * 0.1 + 0.08);
        }
        break;
      }
      case "question": {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "sine";
        osc.frequency.setValueAtTime(523, now);
        osc.frequency.setValueAtTime(659, now + 0.1);
        gain.gain.setValueAtTime(0.06, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.2);
        osc.connect(gain).connect(ctx.destination);
        osc.start(now);
        osc.stop(now + 0.2);
        break;
      }
      case "stop": {
        [880, 660, 440].forEach((freq, i) => {
          const osc = ctx.createOscillator();
          const gain = ctx.createGain();
          osc.type = "triangle";
          osc.frequency.setValueAtTime(freq, now + i * 0.08);
          gain.gain.setValueAtTime(0.06, now + i * 0.08);
          gain.gain.exponentialRampToValueAtTime(0.001, now + i * 0.08 + 0.12);
          osc.connect(gain).connect(ctx.destination);
          osc.start(now + i * 0.08);
          osc.stop(now + i * 0.08 + 0.12);
        });
        break;
      }
      case "allow": {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "triangle";
        osc.frequency.setValueAtTime(440, now);
        osc.frequency.setValueAtTime(554, now + 0.06);
        osc.frequency.setValueAtTime(660, now + 0.1);
        gain.gain.setValueAtTime(0.06, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.2);
        osc.connect(gain).connect(ctx.destination);
        osc.start(now);
        osc.stop(now + 0.2);
        break;
      }
      case "deny": {
        const osc = ctx.createOscillator();
        const gain = ctx.createGain();
        osc.type = "sawtooth";
        osc.frequency.setValueAtTime(150, now);
        osc.frequency.setValueAtTime(100, now + 0.1);
        gain.gain.setValueAtTime(0.06, now);
        gain.gain.exponentialRampToValueAtTime(0.001, now + 0.18);
        osc.connect(gain).connect(ctx.destination);
        osc.start(now);
        osc.stop(now + 0.18);
        break;
      }
    }
  } catch {
    // Silent fail
  }
}

// ─── Agent Helpers ───────────────────────────────────────────────────

function getAgentConfig(agentId?: string) {
  return AGENT_CONFIGS[agentId || "claude-code"] || AGENT_CONFIGS["claude-code"];
}

function setAgentStyle(iconEl: HTMLElement, badgeEl: HTMLElement | null, agentId?: string) {
  const config = getAgentConfig(agentId);
  iconEl.textContent = config.icon;
  iconEl.style.color = config.color;
  if (badgeEl) {
    badgeEl.style.color = config.color;
    badgeEl.style.background = config.color + "18";
    badgeEl.style.borderColor = config.color + "40";
  }
}

// ─── View Management ─────────────────────────────────────────────────

function hideAllViews() {
  idleView.classList.add("hidden");
  notificationView.classList.add("hidden");
  permissionView.classList.add("hidden");
  questionView.classList.add("hidden");
  stopView.classList.add("hidden");
}

function showIdleView() {
  hideAllViews();
  idleView.classList.remove("hidden");
  container.classList.remove("hidden", "fade-out");
  invoke("resize_window", { height: 180 });
  refreshSessions();
}

function showContainer() {
  container.classList.remove("hidden", "fade-out");
}

function returnToIdle(delay = 0) {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    showIdleView();
  }, delay);
}

// ─── Sessions ────────────────────────────────────────────────────────

async function refreshSessions() {
  try {
    const result = await invoke("get_sessions") as { count: number; sessions: Array<{ agent: string; pid: number; cwd: string; session_id: string }> };
    if (result.count === 0) {
      sessionInfo.textContent = "No active agent sessions";
    } else {
      const byAgent = new Map<string, number>();
      for (const s of result.sessions) {
        byAgent.set(s.agent, (byAgent.get(s.agent) || 0) + 1);
      }
      const parts: string[] = [];
      for (const [agent, count] of byAgent) {
        const name = AGENT_CONFIGS[agent]?.name || agent;
        parts.push(`${name} (${count})`);
      }
      sessionInfo.textContent = parts.join(" \u00b7 ");
    }
  } catch {
    sessionInfo.textContent = "";
  }
}

// ─── Tool Input Formatting ───────────────────────────────────────────

function formatToolInput(toolName: string, toolInput: Record<string, unknown>): { title: string; detail: string } {
  let title = toolName;
  let detail = "";

  if (toolName === "Bash" || toolName === "bash") {
    title = `Bash: ${toolInput.command || toolInput.cmd || ""}`;
    detail = (toolInput.description as string) || "";
  } else if (toolName === "Edit" || toolName === "edit") {
    title = `Edit: ${toolInput.file_path || toolInput.file || ""}`;
  } else if (toolName === "Write" || toolName === "write") {
    title = `Write: ${toolInput.file_path || toolInput.file || ""}`;
  } else if (toolName === "Read" || toolName === "read") {
    title = `Read: ${toolInput.file_path || toolInput.file || ""}`;
  } else {
    const firstVal = Object.values(toolInput).find((v) => typeof v === "string");
    if (firstVal) title = `${toolName}: ${firstVal}`;
  }

  return { title, detail };
}

// ─── AskUserQuestion Rendering ───────────────────────────────────────

interface QuestionOption { label: string; description?: string; }
interface Question { question: string; header?: string; options: QuestionOption[]; multiSelect?: boolean; }

function renderQuestionView(questions: Question[]) {
  questionOptions.innerHTML = "";
  questionInputRow.classList.add("hidden");

  const q = questions[0];
  questionText.textContent = q.question;

  const multiSelect = q.multiSelect === true;
  const selectedLabels = new Set<string>();

  q.options.forEach((opt, i) => {
    const btn = document.createElement("button");
    btn.className = "option-btn";

    const keySpan = document.createElement("span");
    keySpan.className = "option-key";
    keySpan.textContent = `${i + 1}`;

    const contentDiv = document.createElement("div");
    contentDiv.className = "option-content";

    const labelSpan = document.createElement("span");
    labelSpan.className = "option-label";
    labelSpan.textContent = opt.label;
    contentDiv.appendChild(labelSpan);

    if (opt.description) {
      const descSpan = document.createElement("span");
      descSpan.className = "option-desc";
      descSpan.textContent = opt.description;
      contentDiv.appendChild(descSpan);
    }

    btn.appendChild(keySpan);
    btn.appendChild(contentDiv);

    btn.addEventListener("click", () => {
      if (multiSelect) {
        if (selectedLabels.has(opt.label)) {
          selectedLabels.delete(opt.label);
          btn.classList.remove("selected");
        } else {
          selectedLabels.add(opt.label);
          btn.classList.add("selected");
        }
      } else {
        submitQuestionAnswer(questions, q.question, opt.label);
      }
    });

    questionOptions.appendChild(btn);
  });

  // "Other" option
  const otherBtn = document.createElement("button");
  otherBtn.className = "option-btn";
  otherBtn.innerHTML = `<span class="option-key">${q.options.length + 1}</span><div class="option-content"><span class="option-label">Other</span><span class="option-desc">Type a custom response</span></div>`;
  otherBtn.addEventListener("click", () => {
    questionInputRow.classList.remove("hidden");
    questionInput.value = "";
    questionInput.focus();
    requestAnimationFrame(() => {
      invoke("resize_window", { height: Math.max(180, container.offsetHeight + 40) });
    });
  });
  questionOptions.appendChild(otherBtn);

  if (multiSelect) {
    const submitBtn = document.createElement("button");
    submitBtn.className = "btn btn-allow";
    submitBtn.style.marginTop = "6px";
    submitBtn.textContent = "Submit Selection";
    submitBtn.addEventListener("click", () => {
      if (selectedLabels.size > 0) {
        submitQuestionAnswer(questions, q.question, Array.from(selectedLabels).join(","));
      }
    });
    questionOptions.appendChild(submitBtn);
  }

  const submitOther = () => {
    const val = questionInput.value.trim();
    if (val) submitQuestionAnswer(questions, q.question, val);
  };
  questionInputSubmit.onclick = submitOther;
  questionInput.onkeydown = (e) => {
    if (e.key === "Enter") { e.preventDefault(); submitOther(); }
  };

  requestAnimationFrame(() => {
    invoke("resize_window", { height: Math.max(180, container.offsetHeight + 20) });
  });
}

function submitQuestionAnswer(questions: Question[], questionKey: string, answer: string) {
  if (!currentQuestionRequestId || !currentQuestionToolInput) return;

  const updatedInput = { ...currentQuestionToolInput };
  const answers = (updatedInput.answers as Record<string, string>) || {};
  answers[questionKey] = answer;
  updatedInput.answers = answers;

  invoke("respond_question", {
    requestId: currentQuestionRequestId,
    permissionDecision: "allow",
    updatedInput,
  });

  currentQuestionRequestId = null;
  currentQuestionToolInput = null;
  playSound("allow");
  if (ghostMode) { showGhostFeedback("answer"); } else { processNextEvent(); }
}

// ─── Event Listeners (Tauri events from Rust backend) ────────────────

// ─── Display Functions (called by queue) ─────────────────────────────

function showNotificationEvent(event: any) {
  if (hideTimer) clearTimeout(hideTimer);
  expand();
  hideAllViews();

  currentSessionId = event.session_id || null;
  setAgentStyle(notificationIcon, notificationBadge, event.agent);
  notificationMessage.textContent = event.message || "Notification";
  notificationView.classList.remove("hidden");
  invoke("resize_window", { height: 180 });
  showContainer();
  playSound("notification");

  // Auto-dismiss then process next in queue
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    processNextEvent();
  }, 15000);
}

function showQuestionEvent(event: any) {
  if (hideTimer) clearTimeout(hideTimer);
  expand();
  hideAllViews();

  currentQuestionRequestId = event.request_id;
  currentQuestionPayload = event;
  currentSessionId = event.session_id || null;
  const toolInput = event.tool_input || {};
  currentQuestionToolInput = toolInput;

  // Show project context
  questionProject.textContent = event.project ? `· ${event.project}` : "";
  updatePendingBadge();

  const questions = toolInput.questions as Question[] | undefined;
  if (Array.isArray(questions) && questions.length > 0) {
    questionView.classList.remove("hidden");
    renderQuestionView(questions);
    showContainer();
    playSound("question");
  } else {
    invoke("respond_question", {
      requestId: event.request_id,
      permissionDecision: "allow",
      updatedInput: null,
    });
    currentQuestionRequestId = null;
    currentQuestionToolInput = null;
    processNextEvent();
  }
}

function showPermissionEvent(event: any) {
  if (hideTimer) clearTimeout(hideTimer);
  expand();
  hideAllViews();

  currentPermissionRequestId = event.request_id;
  currentPermissionToolName = event.tool_name;
  currentPermissionPayload = event;
  currentSessionId = event.session_id || null;

  // Show project context
  permissionProject.textContent = event.project ? `· ${event.project}` : "";
  updatePendingBadge();

  setAgentStyle(permissionIcon, null, event.agent);
  const { title, detail } = formatToolInput(event.tool_name, event.tool_input || {});
  permissionTool.textContent = title;
  permissionDetail.textContent = detail;
  permissionDetail.style.display = detail ? "block" : "none";

  permissionView.classList.remove("hidden");
  invoke("resize_window", { height: 180 });
  showContainer();
  playSound("permission");
}

function showStopEvent(event: any) {
  if (hideTimer) clearTimeout(hideTimer);
  expand();
  hideAllViews();

  currentSessionId = event.session_id || null;
  const config = getAgentConfig(event.agent);
  setAgentStyle(stopIcon, null, event.agent);
  let msg = `${config.name} finished`;
  if (event.duration_ms) {
    const secs = Math.round(event.duration_ms / 1000);
    msg += secs < 60 ? ` (${secs}s)` : ` (${Math.floor(secs / 60)}m ${secs % 60}s)`;
  }
  stopMessage.textContent = msg;
  stopView.classList.remove("hidden");
  invoke("resize_window", { height: 180 });
  showContainer();
  playSound("stop");

  hideTimer = setTimeout(() => {
    processNextEvent();
  }, 15000);
}

// ─── Event Listeners (enqueue incoming events) ───────────────────────

listen("shelly://notification", (e: { payload: any }) => {
  enqueueEvent({ type: "notification", payload: e.payload });
});

listen("shelly://question", (e: { payload: any }) => {
  enqueueEvent({ type: "question", payload: e.payload });
});

listen("shelly://permission", (e: { payload: any }) => {
  enqueueEvent({ type: "permission", payload: e.payload });
});

listen("shelly://stop", (e: { payload: any }) => {
  enqueueEvent({ type: "stop", payload: e.payload });
});

// Dismiss: server detected client disconnect (user answered in terminal)
listen("shelly://dismiss", (e: { payload: any }) => {
  const dismissId = e.payload.request_id;

  // If this is the currently displayed event, skip to next
  if (currentPermissionRequestId === dismissId) {
    currentPermissionRequestId = null;
    currentPermissionToolName = null;
    processNextEvent();
    return;
  }
  if (currentQuestionRequestId === dismissId) {
    currentQuestionRequestId = null;
    currentQuestionToolInput = null;
    processNextEvent();
    return;
  }

  // Otherwise remove it from the queue (hasn't been shown yet)
  const idx = eventQueue.findIndex(
    (ev) => (ev.type === "permission" || ev.type === "question") && ev.payload.request_id === dismissId
  );
  if (idx !== -1) {
    eventQueue.splice(idx, 1);
    updatePendingBadge();
  }
});

// ─── Permission Button Handlers ──────────────────────────────────────

function handleAllow() {
  if (!currentPermissionRequestId) return;
  invoke("respond_permission", { requestId: currentPermissionRequestId, behavior: "allow" });
  currentPermissionRequestId = null;
  currentPermissionToolName = null;
  playSound("allow");
  if (ghostMode) { showGhostFeedback("allow"); } else { processNextEvent(); }
}

function handleAllowAlways() {
  if (!currentPermissionRequestId) return;
  if (currentPermissionToolName) {
    invoke("allow_tool_always", { toolName: currentPermissionToolName });
  }
  invoke("respond_permission", { requestId: currentPermissionRequestId, behavior: "allow" });
  currentPermissionRequestId = null;
  currentPermissionToolName = null;
  playSound("allow");
  if (ghostMode) { showGhostFeedback("allow"); } else { processNextEvent(); }
}

function handleDeny() {
  if (!currentPermissionRequestId) return;
  invoke("respond_permission", { requestId: currentPermissionRequestId, behavior: "deny" });
  currentPermissionRequestId = null;
  currentPermissionToolName = null;
  playSound("deny");
  if (ghostMode) { showGhostFeedback("deny"); } else { processNextEvent(); }
}

btnAllow.addEventListener("click", handleAllow);
btnAllowAlways.addEventListener("click", handleAllowAlways);
btnDeny.addEventListener("click", handleDeny);

// ─── Jump to Terminal ─────────────────────────────────────────────────

async function handleJumpToTerminal() {
  await appWindow.hide();
  try {
    if (currentSessionId) {
      await invoke("jump_to_session", { sessionId: currentSessionId });
    } else {
      const terminals = await invoke("get_terminals") as string[];
      if (terminals.length > 0) {
        await invoke("jump_to_terminal", { terminalApp: terminals[0] });
      }
    }
  } catch {
    // no terminals found
  }
}

btnJumpPerm.addEventListener("click", handleJumpToTerminal);
btnJumpQuestion.addEventListener("click", handleJumpToTerminal);
btnJumpStop.addEventListener("click", handleJumpToTerminal);
btnJumpNotif.addEventListener("click", handleJumpToTerminal);

// ─── Skip (cycle through pending events) ──────────────────────────────

function handleSkip() {
  // Re-queue the current event at the end
  if (currentPermissionRequestId) {
    eventQueue.push({ type: "permission", payload: { ...currentPermissionPayload } });
    currentPermissionRequestId = null;
    currentPermissionToolName = null;
  } else if (currentQuestionRequestId) {
    eventQueue.push({ type: "question", payload: { ...currentQuestionPayload } });
    currentQuestionRequestId = null;
    currentQuestionToolInput = null;
  }
  updatePendingBadge();
  processNextEvent();
}

pendingBadge.addEventListener("click", handleSkip);
pendingBadge.style.cursor = "pointer";

// ─── Keyboard Shortcuts ──────────────────────────────────────────────

document.addEventListener("keydown", (e) => {
  if (currentPermissionRequestId && !currentQuestionRequestId) {
    if (e.metaKey && e.key === "y") { e.preventDefault(); handleAllow(); }
    else if (e.metaKey && e.key === "n") { e.preventDefault(); handleDeny(); }
  }

  if (currentQuestionRequestId) {
    const num = parseInt(e.key, 10);
    if (!e.metaKey && !e.ctrlKey && !e.altKey && num >= 1 && num <= 9) {
      if (document.activeElement === questionInput) return;
      const btns = questionOptions.querySelectorAll(".option-btn");
      if (btns[num - 1]) { e.preventDefault(); (btns[num - 1] as HTMLButtonElement).click(); }
    }
  }

});

// ─── Mute & Theme Toggles ────────────────────────────────────────────

const THEME_ICONS: Record<Theme, string> = {
  glass: "\u25C7",   // ◇ diamond
  white: "\u25CB",   // ○ circle
  dark: "\u25CF",    // ● filled circle
};

function applyMuteState() {
  muteIcon.textContent = isMuted ? "\u{1F507}" : "\u{1F508}";
  btnMute.classList.toggle("active", isMuted);
}

function applyThemeState() {
  container.classList.remove("theme-white", "theme-dark");
  if (currentTheme === "white") container.classList.add("theme-white");
  else if (currentTheme === "dark") container.classList.add("theme-dark");
  themeIcon.textContent = THEME_ICONS[currentTheme];
}

btnMute.addEventListener("click", () => {
  isMuted = !isMuted;
  localStorage.setItem("shelly-muted", String(isMuted));
  applyMuteState();
});

btnTheme.addEventListener("click", () => {
  const idx = THEMES.indexOf(currentTheme);
  currentTheme = THEMES[(idx + 1) % THEMES.length];
  localStorage.setItem("shelly-theme", currentTheme);
  applyThemeState();
});

function updateGhostIcon() {
  // 👻⃠ = ghost mode off (crossed out), 👻 = ghost mode on
  ghostIcon.textContent = ghostMode ? "\u{1F47B}" : "\u{1F47B}\u20E0";
}

function ghostHide() {
  if (!ghostMode) return;
  container.classList.add("ghost-vanishing");
  setTimeout(() => {
    container.classList.remove("ghost-vanishing");
    appWindow.hide();
  }, 500);
}

function showGhostFeedback(type: "allow" | "deny" | "answer") {
  if (!ghostMode) return;

  // Hide main content
  hideAllViews();
  container.style.opacity = "0";
  container.style.transition = "opacity 0.3s ease";

  // Show feedback overlay
  ghostFeedback.className = "ghost-feedback";
  if (type === "allow") {
    ghostFeedback.classList.add("feedback-allow");
    ghostFeedbackIcon.textContent = "\u2714";  // ✔
    ghostFeedbackText.textContent = "ALLOWED";
  } else if (type === "deny") {
    ghostFeedback.classList.add("feedback-deny");
    ghostFeedbackIcon.textContent = "\u2718";  // ✘
    ghostFeedbackText.textContent = "DENIED";
  } else {
    ghostFeedback.classList.add("feedback-answer");
    ghostFeedbackIcon.textContent = "\u2714";  // ✔
    ghostFeedbackText.textContent = "ANSWERED";
  }

  // After animation completes, hide window
  setTimeout(() => {
    ghostFeedback.classList.add("hidden");
    container.style.opacity = "1";
    processNextEvent();
  }, 1400);
}

function expand() {
  appWindow.show();
  invoke("resize_window", { height: 180 });
  if (ghostMode) {
    container.classList.add("ghost-appearing");
    setTimeout(() => container.classList.remove("ghost-appearing"), 350);
  }
}

btnMinimize.addEventListener("click", () => {
  ghostMode = !ghostMode;
  updateGhostIcon();
  if (ghostMode) {
    ghostHide();
  }
});

updateGhostIcon();

// ─── Yolo Mode Toggle ────────────────────────────────────────────────

function updateYoloIcon() {
  // ⚡ = yolo on, ⚡⃠ = yolo off (crossed out)
  yoloIcon.textContent = yoloMode ? "\u26A1" : "\u26A1\u20E0";
  btnYolo.classList.toggle("active", yoloMode);
}

btnYolo.addEventListener("click", () => {
  yoloMode = !yoloMode;
  updateYoloIcon();
  invoke("set_yolo_mode", { enabled: yoloMode });
  playSound(yoloMode ? "allow" : "deny");
});

updateYoloIcon();

btnClose.addEventListener("click", () => {
  appWindow.close();
});

applyMuteState();
applyThemeState();

// ─── Window Dragging ─────────────────────────────────────────────────

const appWindow = getCurrentWindow();

// Make the container draggable — start drag on mousedown unless on a button/input
container.addEventListener("mousedown", (e) => {
  const target = e.target as HTMLElement;
  // Don't drag if clicking on interactive elements
  if (
    target.closest("button") ||
    target.closest("input") ||
    target.closest(".option-btn")
  ) {
    return;
  }
  e.preventDefault();
  appWindow.startDragging();
});

// ─── Init ────────────────────────────────────────────────────────────

showIdleView();
setInterval(refreshSessions, 10000);

// ─── Auto-Update Check ───────────────────────────────────────────────
async function checkForUpdates() {
  try {
    const update = await check();
    if (update) {
      console.log(`Update available: ${update.version}`);
      await update.downloadAndInstall();
      // Show brief notification before relaunch
      hideAllViews();
      notificationMessage.textContent = `Updating to v${update.version}...`;
      notificationView.classList.remove("hidden");
      invoke("resize_window", { height: 180 });
      showContainer();
      playSound("notification");
      setTimeout(() => relaunch(), 2000);
    }
  } catch (e) {
    console.log("Update check failed:", e);
  }
}

// Check for updates 60s after launch
setTimeout(checkForUpdates, 60000);

