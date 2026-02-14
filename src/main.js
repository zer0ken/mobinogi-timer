const { listen } = window.__TAURI__.event;
const { invoke } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

const progress = document.getElementById("progress");
const label = document.getElementById("label");
const timerBar = document.getElementById("timer-bar");

function applyOpacity(opacity) {
  timerBar.style.background = `rgba(30, 30, 30, ${opacity})`;
}

invoke("get_settings").then((settings) => {
  applyOpacity(settings.overlay_opacity);
});

listen("settings-updated", () => {
  invoke("get_settings").then((settings) => {
    applyOpacity(settings.overlay_opacity);
  });
});

listen("timer-update", (event) => {
  const { state, percent, remaining } = event.payload;
  const secs = Math.ceil(remaining);

  if (state === "idle") {
    progress.className = "idle";
    progress.style.width = "100%";
    label.textContent = "준비됨";
  } else if (state === "duration") {
    progress.className = "";
    progress.style.width = percent + "%";
    label.textContent = "각성 " + secs + "s";
  } else if (state === "cooldown") {
    progress.className = "cooldown";
    progress.style.width = percent + "%";
    label.textContent = "쿨다운 " + secs + "s";
  }
});

// Drag to move overlay when mouse events are enabled
document.getElementById("timer-bar").addEventListener("mousedown", () => {
  getCurrentWindow().startDragging();
});
