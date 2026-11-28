import "./style.css";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const isTauri = "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
const card = document.querySelector(".reminder-card") as HTMLDivElement;
const textEl = document.getElementById("reminderText") as HTMLParagraphElement;
const HIDE_AFTER_MS = 10_000;
let hideTimer = 0;

function render(text: string) {
  textEl.textContent = text;
  // Layout is synchronous here, so it also works before the window is shown.
  const height = Math.ceil(card.offsetHeight);
  if (isTauri) {
    void invoke("show_reminder_bubble_ready", { height }).catch(() => undefined);
  }
  if (hideTimer) window.clearTimeout(hideTimer);
  hideTimer = window.setTimeout(() => {
    if (isTauri) void invoke("close_reminder_bubble").catch(() => undefined);
  }, HIDE_AFTER_MS);
}

async function boot() {
  if (!isTauri) {
    render(new URLSearchParams(location.search).get("text") ?? "该喝水啦，起来活动一下。");
    return;
  }
  const text = await invoke<string | null>("get_reminder_text").catch(() => null);
  render(text ?? "");
  await listen<string>("reminder-bubble", (ev) => render(ev.payload));
}

void boot();