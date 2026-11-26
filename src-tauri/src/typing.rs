use crate::config::{app_data_dir, quarantine_corrupt, write_file_atomic};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Keystroke counters by class (letters / digits / function keys). The app never
/// records which keys were pressed, only how many, so the file stays private by
/// construction. Counters live next to settings.json and survive restarts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TypingStats {
    pub total: u64,
    pub letters: u64,
    pub digits: u64,
    pub function_keys: u64,
    pub today_total: u64,
    pub today_letters: u64,
    pub today_digits: u64,
    pub today_function_keys: u64,
    /// Local calendar day (YYYY-MM-DD) that the today_* counters belong to.
    pub day: String,
    /// Transient: counters changed since the last successful disk write.
    #[serde(skip)]
    pub dirty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyClass {
    Letter,
    Digit,
    Function,
    Other,
}

/// Only letters, digits and F-keys are counted; everything else (modifiers,
/// navigation, punctuation, numpad operators) is ignored.
fn classify(code: &str) -> KeyClass {
    if let Some(rest) = code.strip_prefix("Key") {
        return if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_alphabetic()) {
            KeyClass::Letter
        } else {
            KeyClass::Other
        };
    }
    let digit_key = |prefix: &str| {
        code.strip_prefix(prefix)
            .map(|n| n.len() == 1 && n.chars().all(|c| c.is_ascii_digit()))
            .unwrap_or(false)
    };
    if digit_key("Digit") || digit_key("Numpad") {
        return KeyClass::Digit;
    }
    if let Some(number) = code.strip_prefix('F') {
        if let Ok(value) = number.parse::<u32>() {
            if (1..=24).contains(&value) {
                return KeyClass::Function;
            }
        }
    }
    KeyClass::Other
}

/// Local calendar day. Windows uses the real system clock; other targets fall
/// back to a UTC civil date so rollover still happens once per day.
fn local_day() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Time::{GetLocalTime, SYSTEMTIME};
        let mut st: SYSTEMTIME = unsafe { std::mem::zeroed() };
        unsafe { GetLocalTime(&mut st) };
        return format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay);
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let (y, m, d) = civil_from_days((secs / 86400) as i64);
        return format!("{y:04}-{m:02}-{d:02}");
    }
}

/// days-since-epoch to (year, month, day); Howard Hinnant's civil_from_days.
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

impl TypingStats {
    pub fn record(&mut self, code: &str) {
        let class = classify(code);
        if class == KeyClass::Other {
            return;
        }
        let day = local_day();
        if self.day != day {
            self.day = day;
            self.today_total = 0;
            self.today_letters = 0;
            self.today_digits = 0;
            self.today_function_keys = 0;
        }
        self.total += 1;
        self.today_total += 1;
        match class {
            KeyClass::Letter => {
                self.letters += 1;
                self.today_letters += 1;
            }
            KeyClass::Digit => {
                self.digits += 1;
                self.today_digits += 1;
            }
            KeyClass::Function => {
                self.function_keys += 1;
                self.today_function_keys += 1;
            }
            KeyClass::Other => {}
        }
        self.dirty = true;
    }

    pub fn reset(&mut self) {
        *self = Self {
            day: local_day(),
            ..Self::default()
        };
        self.dirty = true;
    }
}

pub fn typing_path() -> PathBuf {
    app_data_dir().join("typing.json")
}

pub fn load_typing_stats() -> Arc<Mutex<TypingStats>> {
    let parsed = fs::read_to_string(typing_path())
        .ok()
        .and_then(|content| match serde_json::from_str::<TypingStats>(&content) {
            Ok(stats) => Some(stats),
            Err(_) => {
                quarantine_corrupt(&typing_path());
                None
            }
        });
    Arc::new(Mutex::new(parsed.unwrap_or_default()))
}

pub fn save_typing_stats(stats: &TypingStats) -> Result<(), String> {
    let json = serde_json::to_string_pretty(stats).map_err(|e| e.to_string())?;
    write_file_atomic(&typing_path(), json.as_bytes()).map_err(|e| e.to_string())
}
