//! Local status bridge.
//!
//! Publishes the activity counters to `%APPDATA%\7paws\status.json` (and the
//! equivalent per-platform data directory) so other local tools — starting
//! with the author's own "work-encourager" Electron app — can read how much
//! typing and pointer travel happened today without any IPC or network.
//!
//! The file is rewritten atomically (temp file + rename), so a reader never
//! observes a half-written document. Only aggregate counts are published:
//! never key names, window titles or pointer positions.

use crate::config::{app_data_dir, write_file_atomic};
use crate::typing::TypingStats;
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How often the counters are refreshed while they change.
const REFRESH_INTERVAL: Duration = Duration::from_secs(1);
/// Idle heartbeat so readers can tell the app is still alive.
const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

/// Estimate centimetres from raw pixels assuming a 96 DPI display, which is
/// Windows' default logical resolution. Real travel depends on the monitor's
/// physical DPI, so this is only an order-of-magnitude convenience.
const CM_PER_PX: f64 = 2.54 / 96.0;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusPayload {
    app: &'static str,
    version: &'static str,
    /// Epoch milliseconds, for machines.
    updated_at_ms: u64,
    /// Same instant as RFC 3339, for humans reading the file.
    updated_at: String,
    /// Whether the settings toggle allows counting at all.
    typing_enabled: bool,
    /// Privacy mode freezes every counter; readers can surface that state.
    privacy_mode: bool,
    typing: TypingPayload,
    mouse: MousePayload,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TypingPayload {
    total: u64,
    letters: u64,
    digits: u64,
    function_keys: u64,
    today_total: u64,
    today_letters: u64,
    today_digits: u64,
    today_function_keys: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MousePayload {
    /// Lifetime pointer travel in raw screen pixels.
    distance_px: u64,
    /// Lifetime wheel detents.
    wheel: u64,
    today_distance_px: u64,
    today_wheel: u64,
    /// Convenience estimates, marked as estimates in the field name.
    distance_meters_estimate: f64,
    today_distance_meters_estimate: f64,
}

fn status_path() -> std::path::PathBuf {
    app_data_dir().join("status.json")
}

fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Seconds precision is plenty for a status file; derive the RFC 3339 text
/// from the same millisecond value instead of pulling in a date crate.
fn rfc3339_from_ms(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, d) = civil_from_days(days);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;
    format!("{y:04}-{m:02}-{d:02}T{hour:02}:{minute:02}:{second:02}Z")
}

/// days-since-epoch to (year, month, day); mirrors typing.rs.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn build_payload(stats: &TypingStats, typing_enabled: bool, privacy_mode: bool) -> StatusPayload {
    let now = epoch_ms();
    StatusPayload {
        app: "7paws",
        version: env!("CARGO_PKG_VERSION"),
        updated_at_ms: now,
        updated_at: rfc3339_from_ms(now),
        typing_enabled,
        privacy_mode,
        typing: TypingPayload {
            total: stats.total,
            letters: stats.letters,
            digits: stats.digits,
            function_keys: stats.function_keys,
            today_total: stats.today_total,
            today_letters: stats.today_letters,
            today_digits: stats.today_digits,
            today_function_keys: stats.today_function_keys,
        },
        mouse: MousePayload {
            distance_px: stats.mouse_distance_px,
            wheel: stats.mouse_wheel,
            today_distance_px: stats.today_mouse_distance_px,
            today_wheel: stats.today_mouse_wheel,
            distance_meters_estimate: stats.mouse_distance_px as f64 * CM_PER_PX / 100.0,
            today_distance_meters_estimate: stats.today_mouse_distance_px as f64
                * CM_PER_PX
                / 100.0,
        },
    }
}

fn write_status(payload: &StatusPayload) {
    match serde_json::to_string_pretty(payload) {
        Ok(json) => {
            if let Err(err) = write_file_atomic(&status_path(), json.as_bytes()) {
                eprintln!("failed to write {}: {err}", status_path().display());
            }
        }
        Err(err) => eprintln!("failed to serialize status payload: {err}"),
    }
}

/// Spawns the publisher thread. Cheap to call more than once: the thread owns
/// its own clocks and exits with the process.
pub fn start(
    typing_stats: Arc<Mutex<TypingStats>>,
    settings: Arc<Mutex<crate::config::AppSettings>>,
) {
    thread::spawn(move || {
        let mut last_written: Option<String> = None;
        let mut last_write = std::time::Instant::now() - HEARTBEAT_INTERVAL;
        loop {
            thread::sleep(REFRESH_INTERVAL);
            let snapshot = match typing_stats.lock() {
                Ok(stats) => stats.clone(),
                Err(_) => continue,
            };
            let (enabled, privacy) = settings
                .lock()
                .map(|s| (s.typing_stats_enabled, s.privacy_mode))
                .unwrap_or((false, false));
            // Serializing to compare costs less than rewriting the file on
            // every tick, and the heartbeat keeps `updatedAt` fresh.
            let payload = build_payload(&snapshot, enabled, privacy);
            let serialized = serde_json::to_string(&payload).ok();
            let changed = serialized.as_deref() != last_written.as_deref();
            let heartbeat_due = last_write.elapsed() >= HEARTBEAT_INTERVAL;
            if changed || heartbeat_due {
                write_status(&payload);
                last_written = serialized;
                last_write = std::time::Instant::now();
            }
        }
    });
}
