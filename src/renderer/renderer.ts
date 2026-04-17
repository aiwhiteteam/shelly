// Agent config — Open Island palette
const AGENT_CONFIGS: Record<string, { name: string; color: string; icon: string }> = {
  "claude-code": { name: "Claude Code", color: "#6E9FFF", icon: "\u26A1" },
  "codex-cli": { name: "Codex CLI", color: "#42E86B", icon: "\u25C6" },
  "gemini-cli": { name: "Gemini CLI", color: "#6E9FFF", icon: "\u2726" },
  cursor: { name: "Cursor", color: "#FFB547", icon: "\u25B8" },
  opencode: { name: "OpenCode", color: "#FFB547", icon: "\u25CB" },
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
const btnYolo = document.getElementById("btn-yolo")! as HTMLButtonElement;
const yoloIcon = document.getElementById("yolo-icon")!;
const btnMinimize = document.getElementById("btn-minimize")! as HTMLButtonElement;
const ghostIcon = document.getElementById("ghost-icon")!;
const btnClose = document.getElementById("btn-close")! as HTMLButtonElement;
const pendingBadge = document.getElementById("pending-badge")!;
const ghostFeedback = document.getElementById("ghost-feedback")!;
const ghostFeedbackIcon = document.getElementById("ghost-feedback-icon")!;
const ghostFeedbackText = document.getElementById("ghost-feedback-text")!;
const scoutLogo = document.getElementById("scout-logo")!;
const pixelGlyphCanvas = document.getElementById("pixel-glyph")! as HTMLCanvasElement;
const sessionBadge = document.getElementById("session-badge")!;
const attentionDot = document.getElementById("attention-dot")!;
const btnDismissNotif = document.getElementById("btn-dismiss-notif")! as HTMLButtonElement;
const btnDismissStop = document.getElementById("btn-dismiss-stop")! as HTMLButtonElement;
const btnMinimizePerm = document.getElementById("btn-minimize-perm")! as HTMLButtonElement;
const btnMinimizeQuestion = document.getElementById("btn-minimize-question")! as HTMLButtonElement;

// State
let currentPermissionRequestId: string | null = null;
let currentPermissionToolName: string | null = null;
let currentPermissionPayload: any = null;
let currentQuestionRequestId: string | null = null;
let currentQuestionToolInput: Record<string, unknown> | null = null;
let currentQuestionPayload: any = null;
let currentSessionId: string | null = null;
let hideTimer: ReturnType<typeof setTimeout> | null = null;
let isCompact = true;

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

function updateAttentionDot() {
  const hasInteractive = isProcessingEvent || hasPendingInteractive();
  if (hasInteractive) {
    attentionDot.classList.remove("hidden");
    // Color based on type of pending event
    const pendingType = currentPermissionRequestId ? "permission" :
      currentQuestionRequestId ? "question" :
      eventQueue.find(e => e.type === "permission") ? "permission" :
      eventQueue.find(e => e.type === "question") ? "question" : null;
    attentionDot.classList.remove("phase-question", "phase-running");
    if (pendingType === "question") attentionDot.classList.add("phase-question");
  } else {
    attentionDot.classList.add("hidden");
  }
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
  updateAttentionDot();
}

function processNextEvent() {
  if (eventQueue.length === 0) {
    isProcessingEvent = false;
    updatePendingBadge();
    updateAttentionDot();
    if (ghostMode) {
      ghostHide();
    } else {
      hideToEdge();
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
let ghostMode = false;
let yoloMode = false;

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

// Island shape is handled by CSS border-radius (flat top, rounded bottom)
// No clip-path needed — flush top edge + rounded bottom = Dynamic Island look

// ─── Scout Logo (8x8 pixel grid) ────────────────────────────────────

const SCOUT_PATTERN = [
  "...BBBB...",
  "..BHHHHB..",
  ".BHEEEEHB.",
  "BHEBBBEHB.",
  "BHEBEEEHB.",
  "BHEBBBBB..",
  ".BHEEEEE..",
  "..BBBBBB..",
];

function initScoutLogo() {
  scoutLogo.innerHTML = "";
  for (let y = 0; y < 8; y++) {
    for (let x = 0; x < 10; x++) {
      const cell = document.createElement("div");
      cell.className = "cell";
      const ch = SCOUT_PATTERN[y][x];
      if (ch === "B") cell.classList.add("cell-b");
      else if (ch === "H") cell.classList.add("cell-h");
      else if (ch === "E") cell.classList.add("cell-e");
      scoutLogo.appendChild(cell);
    }
  }
}

function setScoutActive(active: boolean) {
  scoutLogo.classList.toggle("active", active);
}

// ─── Pixel Glyph Animation (animated bar chart) ─────────────────────

const GLYPH_FRAMES: Record<string, number[][][]> = {
  bars: [
    [[1,3,2,1], [2,3,1]],
    [[2,2,3,1], [1,2,3]],
    [[1,2,1,3], [3,1,2]],
    [[3,1,2,2], [2,3,1]],
  ],
  steps: [
    [[1,2,3,4], [1,2,3]],
    [[2,3,4,3], [2,3,2]],
    [[1,2,3,4], [3,2,1]],
    [[2,3,2,1], [2,3,4]],
  ],
  blocks: [
    [[2,4,4,2], [2,4,2]],
    [[3,4,3,2], [3,4,2]],
    [[2,3,4,3], [2,4,3]],
    [[2,4,3,2], [3,4,2]],
  ],
};

let glyphStyle: "bars" | "steps" | "blocks" = "bars";
let glyphFrame = 0;
let glyphTint = "#6E9FFF"; // default claude blue

function drawPixelGlyph() {
  const canvas = pixelGlyphCanvas;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;

  const dpr = window.devicePixelRatio || 1;
  const w = 26;
  const h = 14;
  canvas.width = w * dpr;
  canvas.height = h * dpr;
  canvas.style.width = w + "px";
  canvas.style.height = h + "px";
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, w, h);

  const frames = GLYPH_FRAMES[glyphStyle];
  const frame = frames[glyphFrame % frames.length];
  const [cluster1, cluster2] = frame;

  const pixelSize = 2.4;
  const spacing = 1.1;
  const maxRows = 4;
  const gapBetweenClusters = 3;

  // Parse hex color to RGB
  const r = parseInt(glyphTint.slice(1, 3), 16);
  const g = parseInt(glyphTint.slice(3, 5), 16);
  const b = parseInt(glyphTint.slice(5, 7), 16);

  function drawColumn(colHeights: number[], startX: number, isLastCol: boolean) {
    colHeights.forEach((height, colIdx) => {
      const x = startX + colIdx * (pixelSize + spacing);
      for (let row = 0; row < maxRows; row++) {
        const fromBottom = maxRows - 1 - row;
        if (fromBottom < height) {
          const y = row * (pixelSize + spacing);
          const rowOpacity = 0.45 + ((row + 1) / maxRows) * 0.50;
          const colOpacity = (isLastCol && colIdx === colHeights.length - 1) ? 0.86 : 1.0;
          const alpha = rowOpacity * colOpacity;

          ctx.fillStyle = `rgba(${r}, ${g}, ${b}, ${alpha})`;
          ctx.beginPath();
          // Rounded rect approximation
          const cr = 0.4;
          ctx.moveTo(x + cr, y);
          ctx.lineTo(x + pixelSize - cr, y);
          ctx.quadraticCurveTo(x + pixelSize, y, x + pixelSize, y + cr);
          ctx.lineTo(x + pixelSize, y + pixelSize - cr);
          ctx.quadraticCurveTo(x + pixelSize, y + pixelSize, x + pixelSize - cr, y + pixelSize);
          ctx.lineTo(x + cr, y + pixelSize);
          ctx.quadraticCurveTo(x, y + pixelSize, x, y + pixelSize - cr);
          ctx.lineTo(x, y + cr);
          ctx.quadraticCurveTo(x, y, x + cr, y);
          ctx.fill();
        }
      }
    });
  }

  const cluster1Width = cluster1.length * (pixelSize + spacing);
  drawColumn(cluster1, 0, false);
  drawColumn(cluster2, cluster1Width + gapBetweenClusters, true);

  // Glow effect via shadow
  ctx.shadowColor = `rgba(${r}, ${g}, ${b}, 0.55)`;
  ctx.shadowBlur = 2.2;
}

let glyphInterval: ReturnType<typeof setInterval> | null = null;

function startPixelGlyph() {
  drawPixelGlyph();
  if (glyphInterval) clearInterval(glyphInterval);
  glyphInterval = setInterval(() => {
    glyphFrame = (glyphFrame + 1) % 4;
    drawPixelGlyph();
  }, 180);
}

// ─── Idle Edge / Compact / Expanded Mode ─────────────────────────────

function hideToEdge() {
  isCompact = true;
  container.classList.remove("compact", "expanded");
  container.classList.add("idle-edge");
  hideAllViews();
  setScoutActive(false);
  invoke("resize_window", { height: 14 });
  refreshSessions();
}

function collapse() {
  isCompact = true;
  container.classList.remove("expanded", "idle-edge");
  container.classList.add("compact");
  hideAllViews();
  setScoutActive(false);
  invoke("resize_window", { height: 48 });
  refreshSessions();
  // Auto-hide to edge after 5s of idle
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    if (!isProcessingEvent) hideToEdge();
  }, 5000);
}

function expand() {
  if (hideTimer) clearTimeout(hideTimer);
  isCompact = false;
  container.classList.remove("compact", "idle-edge");
  container.classList.add("expanded");
  setScoutActive(true);
  appWindow.show();
}

// ─── View Management ─────────────────────────────────────────────────

function hideAllViews() {
  [idleView, notificationView, permissionView, questionView, stopView].forEach(v => {
    v.classList.add("hidden");
    v.classList.remove("entering");
  });
}

/** Show a view with a slide-up fade-in animation */
function showViewAnimated(view: HTMLElement) {
  view.classList.remove("hidden");
  view.classList.add("entering");
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      view.classList.remove("entering");
    });
  });
}

function showIdleView() {
  hideToEdge();
}

function showExpandedIdle() {
  expand();
  hideAllViews();
  showViewAnimated(idleView);
  invoke("resize_window", { height: 180 });
  showContainer();
  refreshSessions();
  // Auto-collapse after 10s if no events arrive
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    if (!isProcessingEvent) collapse();
  }, 5000);
}

function showContainer() {
  container.classList.remove("hidden", "fade-out");
}

function returnToIdle(delay = 0) {
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    collapse();
  }, delay);
}

// ─── Sessions ────────────────────────────────────────────────────────

async function refreshSessions() {
  try {
    const result = await invoke("get_sessions") as { count: number; sessions: Array<{ agent: string; pid: number; cwd: string; session_id: string }> };
    if (result.count === 0) {
      sessionInfo.textContent = "No active agent sessions";
      sessionBadge.classList.add("hidden");
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

      // Update compact session badge
      sessionBadge.textContent = String(result.count);
      sessionBadge.classList.remove("hidden");
    }
  } catch {
    sessionInfo.textContent = "";
    sessionBadge.classList.add("hidden");
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

// ─── Display Functions (called by queue) ─────────────────────────────

function showNotificationEvent(event: any) {
  if (hideTimer) clearTimeout(hideTimer);
  expand();
  hideAllViews();

  currentSessionId = event.session_id || null;
  setAgentStyle(notificationIcon, notificationBadge, event.agent);
  notificationMessage.textContent = event.message || "Notification";
  showViewAnimated(notificationView);
  invoke("resize_window", { height: 180 });
  if (!ghostMode) showContainer();
  playSound("notification");

  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    processNextEvent();
  }, 5000);
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

  questionProject.textContent = event.project ? `\u00b7 ${event.project}` : "";
  updatePendingBadge();

  const questions = toolInput.questions as Question[] | undefined;
  if (Array.isArray(questions) && questions.length > 0) {
    showViewAnimated(questionView);
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

  permissionProject.textContent = event.project ? `\u00b7 ${event.project}` : "";
  updatePendingBadge();

  setAgentStyle(permissionIcon, null, event.agent);
  const { title, detail } = formatToolInput(event.tool_name, event.tool_input || {});
  permissionTool.textContent = title;
  permissionDetail.textContent = detail;
  permissionDetail.style.display = detail ? "block" : "none";

  showViewAnimated(permissionView);
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
  showViewAnimated(stopView);
  invoke("resize_window", { height: 180 });
  if (!ghostMode) showContainer();
  playSound("stop");

  hideTimer = setTimeout(() => {
    processNextEvent();
  }, 5000);
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

// Reopen: dock icon clicked — expand from edge
listen("shelly://reopen", () => {
  if (eventQueue.length > 0) {
    processNextEvent();
  } else {
    collapse(); // at least show compact bar
  }
});

// Error: server failed to start (e.g. port conflict)
listen("shelly://error", (e: { payload: any }) => {
  expand();
  hideAllViews();
  notificationMessage.textContent = e.payload.message || "Server error";
  notificationView.classList.remove("hidden");
  invoke("resize_window", { height: 180 });
  showContainer();
  playSound("deny");
});

// Dismiss: server detected client disconnect (user answered in terminal)
listen("shelly://dismiss", (e: { payload: any }) => {
  const dismissId = e.payload.request_id;

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

// Dismiss buttons — immediately skip to next event
function handleDismiss() {
  if (hideTimer) clearTimeout(hideTimer);
  processNextEvent();
}
btnDismissNotif.addEventListener("click", handleDismiss);
btnDismissStop.addEventListener("click", handleDismiss);

// Minimize buttons — re-queue event and collapse to compact bar
function handleMinimize() {
  if (currentPermissionRequestId) {
    eventQueue.push({ type: "permission", payload: { ...currentPermissionPayload } });
    currentPermissionRequestId = null;
    currentPermissionToolName = null;
  } else if (currentQuestionRequestId) {
    eventQueue.push({ type: "question", payload: { ...currentQuestionPayload } });
    currentQuestionRequestId = null;
    currentQuestionToolInput = null;
  }
  isProcessingEvent = false;
  updatePendingBadge();
  collapse();
}
btnMinimizePerm.addEventListener("click", handleMinimize);
btnMinimizeQuestion.addEventListener("click", handleMinimize);

// ─── Skip (cycle through pending events) ──────────────────────────────

function handleSkip() {
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

// ─── Mute Toggle ─────────────────────────────────────────────────────

function applyMuteState() {
  muteIcon.textContent = isMuted ? "\u{1F507}" : "\u{1F508}";
  btnMute.classList.toggle("active", isMuted);
}

btnMute.addEventListener("click", () => {
  isMuted = !isMuted;
  localStorage.setItem("shelly-muted", String(isMuted));
  applyMuteState();
});

// ─── Ghost Mode ──────────────────────────────────────────────────────

function updateGhostIcon() {
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

  hideAllViews();
  container.style.opacity = "0";
  container.style.transition = "opacity 0.3s ease";

  ghostFeedback.className = "ghost-feedback";
  if (type === "allow") {
    ghostFeedback.classList.add("feedback-allow");
    ghostFeedbackIcon.textContent = "\u2714";
    ghostFeedbackText.textContent = "ALLOWED";
  } else if (type === "deny") {
    ghostFeedback.classList.add("feedback-deny");
    ghostFeedbackIcon.textContent = "\u2718";
    ghostFeedbackText.textContent = "DENIED";
  } else {
    ghostFeedback.classList.add("feedback-answer");
    ghostFeedbackIcon.textContent = "\u2714";
    ghostFeedbackText.textContent = "ANSWERED";
  }

  setTimeout(() => {
    ghostFeedback.classList.add("hidden");
    container.style.opacity = "1";
    processNextEvent();
  }, 1400);
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

// ─── Window Dragging ─────────────────────────────────────────────────

const appWindow = getCurrentWindow();

// Click idle-edge → show compact bar, click compact bar → expand (pending events or idle)
container.addEventListener("click", (e) => {
  const target = e.target as HTMLElement;
  if (target.closest("button") || target.closest("input") || target.closest(".option-btn")) return;
  if (container.classList.contains("idle-edge")) {
    collapse(); // edge → compact
  } else if (isCompact) {
    // If there are pending events, show them; otherwise show idle view
    if (eventQueue.length > 0) {
      processNextEvent();
    } else {
      showExpandedIdle();
    }
  }
});

container.addEventListener("mousedown", (e) => {
  const target = e.target as HTMLElement;
  if (
    target.closest("button") ||
    target.closest("input") ||
    target.closest(".option-btn") ||
    target.closest("canvas")
  ) {
    return;
  }
  // Only drag in expanded mode; compact mode uses click-to-expand
  if (isCompact) return;
  e.preventDefault();
  appWindow.startDragging();
});

// ─── Init ────────────────────────────────────────────────────────────

initScoutLogo();
startPixelGlyph();
hideToEdge();
setInterval(refreshSessions, 10000);

// ─── Auto-Update Check ───────────────────────────────────────────────
async function checkForUpdates() {
  try {
    const update = await check();
    if (update) {
      console.log(`Update available: ${update.version}`);
      await update.downloadAndInstall();
      hideAllViews();
      expand();
      notificationMessage.textContent = `Updating to v${update.version}...`;
      showViewAnimated(notificationView);
      invoke("resize_window", { height: 180 });
      showContainer();
      playSound("notification");
      setTimeout(() => relaunch(), 2000);
    }
  } catch (e) {
    console.log("Update check failed:", e);
  }
}

setTimeout(checkForUpdates, 60000);
