const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const progress = document.getElementById("progress");
const label = document.getElementById("label");
const time = document.getElementById("time");
const bg = document.getElementById("bg");
const barTrack = document.getElementById("bar-track");

const appWindow = getCurrentWindow();
const PAD = 8;

function updateBg() {
  const labelRect = label.getBoundingClientRect();
  const barRect = barTrack.getBoundingClientRect();

  let left = labelRect.left - PAD;
  let right = barRect.right + PAD;

  if (time.textContent) {
    const timeRect = time.getBoundingClientRect();
    right = timeRect.right + PAD;
  }

  bg.style.left = left + "px";
  bg.style.width = (right - left) + "px";
}

function applySettings(settings) {
  bg.style.background = `rgba(30, 30, 30, ${settings.overlay_opacity})`;
  barTrack.style.width = settings.overlay_width + "px";
  requestAnimationFrame(updateBg);
}

invoke("get_settings").then((settings) => {
  bg.style.transition = "none";
  applySettings(settings);
  requestAnimationFrame(() => {
    bg.style.transition = "";
  });
});

listen("settings-updated", () => {
  invoke("get_settings").then(applySettings);
});

listen("timer-update", (event) => {
  const { state, percent, remaining, emblem } = event.payload;
  const secs = Math.ceil(remaining);

  if (state === "idle") {
    progress.className = "idle";
    progress.style.width = "100%";
    progress.style.background = "";
    label.textContent = "각성";
    time.textContent = "";
  } else if (state === "duration") {
    progress.className = "";
    progress.style.background = remaining <= 10 ? "#f44336" : "#64D2FF";
    progress.style.width = percent + "%";
    label.textContent = emblem || "각성";
    time.textContent = secs + "s";
  } else if (state === "cooldown") {
    progress.className = "cooldown";
    progress.style.background = "";
    progress.style.width = percent + "%";
    label.textContent = "쿨다운";
    time.textContent = secs + "s";
  }

  requestAnimationFrame(updateBg);
});

bg.addEventListener("mousedown", () => {
  appWindow.startDragging();
});
