const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;
const { LogicalSize } = window.__TAURI__.dpi;

const progress = document.getElementById("progress");
const label = document.getElementById("label");
const time = document.getElementById("time");
const timerBar = document.getElementById("timer-bar");
const barTrack = document.getElementById("bar-track");

const appWindow = getCurrentWindow();

// --- Layout: keep progress bar at a fixed screen position ---

const ALL_LABELS = [
  "각성 준비됨", "대마법사", "무자비한 포식자", "녹아내린 대지",
  "흩날리는 검", "갈라진 땅", "아득한 빛", "부서진 하늘",
  "산맥 군주", "쿨다운", "각성"
];

let maxLabelWidth = 0;
let maxTimeWidth = 0;
let currentBarWidth = 200;

function measureMaxWidths() {
  const m = document.createElement("span");
  const s = getComputedStyle(label);
  m.style.font = s.font;
  m.style.fontWeight = s.fontWeight;
  m.style.fontSize = s.fontSize;
  m.style.fontFamily = s.fontFamily;
  m.style.visibility = "hidden";
  m.style.position = "absolute";
  m.style.whiteSpace = "nowrap";
  document.body.appendChild(m);

  for (const t of ALL_LABELS) {
    m.textContent = t;
    maxLabelWidth = Math.max(maxLabelWidth, m.offsetWidth);
  }

  m.textContent = "00s";
  maxTimeWidth = m.offsetWidth;

  document.body.removeChild(m);
}

function updateWindowSize() {
  const pad = 8, gap = 6;
  const w = pad + maxLabelWidth + gap + currentBarWidth + gap + maxTimeWidth + pad;
  appWindow.setSize(new LogicalSize(Math.ceil(w), 30));
}

function repositionBar() {
  timerBar.style.marginLeft = (maxLabelWidth - label.offsetWidth) + "px";
}

function applySettings(settings) {
  timerBar.style.background = `rgba(30, 30, 30, ${settings.overlay_opacity})`;
  barTrack.style.width = settings.overlay_width + "px";
  currentBarWidth = settings.overlay_width;
  updateWindowSize();
  repositionBar();
}

measureMaxWidths();
invoke("get_settings").then(applySettings);

listen("settings-updated", () => {
  invoke("get_settings").then(applySettings);
});

listen("timer-update", (event) => {
  const { state, percent, remaining, emblem } = event.payload;
  const secs = Math.ceil(remaining);

  if (state === "idle") {
    progress.className = "idle";
    progress.style.width = "100%";
    label.textContent = "각성 준비됨";
    time.textContent = "";
  } else if (state === "duration") {
    progress.className = "";
    progress.style.width = percent + "%";
    label.textContent = emblem || "각성";
    time.textContent = secs + "s";
  } else if (state === "cooldown") {
    progress.className = "cooldown";
    progress.style.width = percent + "%";
    label.textContent = "쿨다운";
    time.textContent = secs + "s";
  }

  repositionBar();
});

timerBar.addEventListener("mousedown", () => {
  appWindow.startDragging();
});
