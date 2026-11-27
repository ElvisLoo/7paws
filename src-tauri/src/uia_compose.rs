//! Reads text committed into the focused edit field through UI Automation so
//! the pet can show real characters (Chinese from WeType or any other IME)
//! instead of raw keycodes, and count Han characters as words.
//!
//! Windows only: IMEs such as WeType are pure TSF and expose nothing through
//! IMM32, while UI Automation reads the focused control cross-process without
//! injection or elevation. On other platforms this module is a no-op.
//!
//! Only TextPattern is used, and only Han ideographs plus CJK punctuation are
//! kept. Chromium/WebView-style fields expose the raw composition buffer
//! (pinyin) between keystrokes, so filtering out everything else makes the
//! watcher show committed text only; ValuePattern is deliberately avoided
//! because document elements hand back whole-page markup through it.
//!
//! Privacy: the watcher counts Han characters only and never stores or
//! forwards the text itself; password fields are skipped entirely, and the
//! whole thing stops while privacy mode is enabled.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};
#[cfg(windows)]
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    UIA_TextPatternId,
};

use crate::config::AppSettings;
use crate::typing::TypingStats;

#[derive(serde::Serialize, Clone)]
struct ComposedPayload {
    text: String,
}

/// Spawns the watcher thread. Any failure inside (COM init, registration) just
/// leaves the feature off; the pet keeps working.
#[cfg(windows)]
pub fn start(app: AppHandle, settings: Arc<Mutex<AppSettings>>, typing_stats: Arc<Mutex<TypingStats>>) {
    std::thread::spawn(move || run(app, settings, typing_stats));
}

#[cfg(not(windows))]
pub fn start(_app: AppHandle, _settings: Arc<Mutex<AppSettings>>, _typing_stats: Arc<Mutex<TypingStats>>) {}

#[cfg(windows)]
fn run(
    app: AppHandle,
    settings: Arc<Mutex<AppSettings>>,
    typing_stats: Arc<Mutex<TypingStats>>,
) {
    // DocumentRange exposes the whole document, which can be huge; the cap
    // keeps each 200 ms poll cheap and bounded.
    const MAX_CHARS: i32 = 2000;
    const POLL_INTERVAL: Duration = Duration::from_millis(200);

    unsafe {
        // The poller gets its own MTA apartment so it never depends on the
        // app's STA threads.
        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return;
        }
        let automation: IUIAutomation =
            match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(automation) => automation,
                Err(_) => return,
            };

        let mut baseline: Option<String> = None;
        let mut last_save = Instant::now();
        let mut last_emit: Option<TypingStats> = None;

        loop {
            std::thread::sleep(POLL_INTERVAL);

            let enabled = settings
                .lock()
                .map(|s| s.compose_tracking_enabled && !s.privacy_mode)
                .unwrap_or(false);
            let current = if enabled {
                read_focused_text(&automation, MAX_CHARS).as_deref().map(meaningful)
            } else {
                // Drop the baseline while disabled so re-enabling does not
                // replay text typed before the toggle.
                baseline = None;
                None
            };

            match (&baseline, current.as_deref()) {
                (Some(old), Some(next)) if next == old.as_str() => {}
                (Some(old), Some(next)) => {
                    // Only strict growth of the visible text counts as new
                    // typing; edits, cursor moves and deletions reset the
                    // baseline silently so no phantom text is ever shown.
                    if let Some(tail) = next.strip_prefix(old.as_str()) {
                        if !tail.is_empty() {
                            let han_count =
                                tail.chars().filter(|c| is_han(*c)).count() as u64;
                            if han_count > 0 {
                                if let Ok(mut stats) = typing_stats.lock() {
                                    stats.record_composed(han_count);
                                }
                            }
                            let _ = app.emit(
                                "cat-composed",
                                ComposedPayload { text: tail.to_string() },
                            );
                        }
                    }
                    baseline = Some(next.to_string());
                }
                (None, Some(next)) => {
                    // First observation after (re)focusing a field: establish
                    // the baseline without counting pre-existing text.
                    baseline = Some(next.to_string());
                }
                (_, None) => {
                    baseline = None;
                }
            }

            // Flush regardless of text changes so counters persisted by this
            // thread survive even when the user stops mid-edit.
            crate::input_hook::flush_typing_stats(&app, &typing_stats, &mut last_save, &mut last_emit);
        }
    }
}

/// Committed text of the focused element, or None when there is no readable
/// text control (or it is a password field).
#[cfg(windows)]
unsafe fn read_focused_text(automation: &IUIAutomation, max_chars: i32) -> Option<String> {
    let element: IUIAutomationElement = automation.GetFocusedElement().ok()?;
    if element.CurrentIsPassword().unwrap_or_default().as_bool() {
        return None;
    }

    // Edit/Document controls expose their content through TextPattern; some
    // also carry ValuePattern, but its value is page markup for documents, so
    // it is intentionally not consulted.
    if let Ok(pattern) = element.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) {
        if let Ok(range) = pattern.DocumentRange() {
            if let Ok(text) = range.GetText(max_chars) {
                return Some(text.to_string());
            }
        }
    }

    None
}

/// Han ideographs plus CJK punctuation: what the pet ends up showing. Raw IME
/// syllables (pinyin letters, apostrophe separators) are dropped here.
fn meaningful(text: &str) -> String {
    text.chars()
        .filter(|c| is_han(*c) || is_cjk_punctuation(*c))
        .collect()
}

fn is_han(c: char) -> bool {
    matches!(
        c,
        '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}'
    )
}

fn is_cjk_punctuation(c: char) -> bool {
    matches!(
        c,
        '\u{3000}'..='\u{303F}' | '\u{FF01}'..='\u{FF65}' | '\u{FFE0}'..='\u{FFE6}'
    )
}
