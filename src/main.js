const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

const progress = document.getElementById("progress");
const label = document.getElementById("label");

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
