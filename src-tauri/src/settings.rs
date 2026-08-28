//! ReminderSettings + SettingsStore 的 Rust 移植。
//!
//! 与原版一致的关键语义：
//! - 写入时对越界值**钳制**（clamp）后再持久化；
//! - 从磁盘读取时对非法值**回退默认**（fallback，不是钳到边界）；
//! - `wallpaperImageURLString` 归一化：trim 后必须是合法 http(s) URL 且
//!   不等于默认图源地址，否则存空串。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const DEFAULT_WALLPAPER_URL: &str = "https://picsum.photos/2880/1800";

/// 设置项合法范围（与 ReminderSettings.swift 完全一致）。
pub mod ranges {
    pub const INTERVAL: (i64, i64) = (10, 180);
    pub const BREAK_DURATION: (i64, i64) = (1, 30);
    pub const SNOOZE: (i64, i64) = (1, 15);
    pub const IDLE_THRESHOLD: (i64, i64) = (1, 10);
    /// 自动倒计时秒数范围；0 = 提醒弹出后不显示倒计时、直接自动开始休息。
    pub const AUTO_START_DELAY: (i64, i64) = (0, 300);
    pub const WINDOW_OPACITY: (f64, f64) = (0.3, 1.0);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WallpaperTheme {
    Nature,
    CityNight,
    Minimal,
    Cartoon,
}

impl Default for WallpaperTheme {
    fn default() -> Self {
        Self::Nature
    }
}

fn clamp_i(v: i64, r: (i64, i64)) -> i64 {
    v.clamp(r.0, r.1)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub reminder_interval_minutes: i64,
    pub break_duration_minutes: i64,
    pub snooze_minutes: i64,
    #[serde(rename = "wallpaperImageURLString")]
    pub wallpaper_image_url_string: String,
    pub wallpaper_theme: WallpaperTheme,
    pub launch_at_login: bool,
    pub sound_enabled: bool,
    pub overlay_other_windows: bool,
    pub activity_based_timing: bool,
    pub idle_threshold_minutes: i64,
    pub auto_start_break: bool,
    pub auto_start_break_delay_seconds: i64,
    pub reminder_window_opacity: f64,
    /// 语言（english / chinese），随设置一并持久化。
    pub app_language: String,
    /// 「暂不提醒」的更新版本（内部字段，仅抑制启动自检弹窗）。
    #[serde(rename = "skippedUpdateVersion")]
    pub skipped_update_version: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            reminder_interval_minutes: 45,
            break_duration_minutes: 5,
            snooze_minutes: 5,
            wallpaper_image_url_string: String::new(),
            wallpaper_theme: WallpaperTheme::default(),
            launch_at_login: true,
            sound_enabled: true,
            overlay_other_windows: false,
            activity_based_timing: true,
            idle_threshold_minutes: 2,
            auto_start_break: true,
            auto_start_break_delay_seconds: 0,
            reminder_window_opacity: 1.0,
            app_language: "chinese".into(),
            skipped_update_version: String::new(),
        }
    }
}

impl Settings {
    /// http(s) URL 校验：scheme 小写化后为 http/https 且 host 非空。
    pub fn is_http_url(s: &str) -> bool {
        let Ok(url) = url::Url::parse(s.trim()) else {
            return false;
        };
        let scheme_ok = matches!(url.scheme(), "http" | "https");
        scheme_ok && url.host_str().is_some_and(|h| !h.is_empty())
    }

    /// 存储字符串归一化：trim；非法或恰好等于默认地址 → ""。
    pub fn normalize_url_string(raw: &str) -> String {
        let t = raw.trim();
        if t.is_empty() {
            return String::new();
        }
        if Self::is_http_url(t) && t != DEFAULT_WALLPAPER_URL {
            t.to_string()
        } else {
            String::new()
        }
    }
}

// 注：Cargo.toml 中未引入 url crate，这里用纯手写解析避免多余依赖。
mod url {
    /// 极简 URL 解析：scheme://host/...；满足本项目校验需求。
    pub struct Url {
        scheme: String,
        host: Option<String>,
    }
    impl Url {
        pub fn parse(input: &str) -> Result<Self, ()> {
            let (scheme, rest) = input.split_once("://").ok_or(())?;
            let host_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let host = &rest[..host_end];
            // host 里不允许再出现 :// 之类的乱序（userinfo@ 视作合法一部分，宽松处理端口）
            let host_clean = host.rsplit('@').next().unwrap_or(host);
            Ok(Self {
                scheme: scheme.to_ascii_lowercase(),
                host: Some(host_clean.to_string()),
            })
        }
        pub fn scheme(&self) -> &str {
            &self.scheme
        }
        pub fn host_str(&self) -> Option<&str> {
            self.host.as_deref()
        }
    }
}

/// JSON 持久化文件：load 具备"非法回退默认"语义，save 假定传入值已钳制。
pub struct SettingsFile {
    path: PathBuf,
}

impl SettingsFile {
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("settings.json"),
        }
    }

    pub fn load(&self) -> Settings {
        let defaults = Settings::default();
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return defaults;
        };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return defaults;
        };

        let in_i = |n: i64, range: (i64, i64)| n >= range.0 && n <= range.1;
        let get_i = |key: &str, fallback: i64, range: (i64, i64)| -> i64 {
            v.get(key)
                .and_then(|x| x.as_i64())
                .filter(|n| in_i(*n, range))
                .unwrap_or(fallback)
        };
        let opacity_fallback = defaults.reminder_window_opacity;
        let opacity = v
            .get("reminderWindowOpacity")
            .and_then(|x| x.as_f64())
            .filter(|f| f >= &ranges::WINDOW_OPACITY.0 && f <= &ranges::WINDOW_OPACITY.1)
            .unwrap_or(opacity_fallback);
        let theme = v
            .get("wallpaperTheme")
            .and_then(|x| x.as_str())
            .and_then(parse_theme)
            .unwrap_or(defaults.wallpaper_theme);
        let lang = v
            .get("appLanguage")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| defaults.app_language.clone());
        let skipped = v
            .get("skippedUpdateVersion")
            .and_then(|x| x.as_str())
            .map(str::to_string)
            .unwrap_or_default();

        Settings {
            reminder_interval_minutes: get_i(
                "reminderIntervalMinutes",
                defaults.reminder_interval_minutes,
                ranges::INTERVAL,
            ),
            break_duration_minutes: get_i(
                "breakDurationMinutes",
                defaults.break_duration_minutes,
                ranges::BREAK_DURATION,
            ),
            snooze_minutes: get_i("snoozeMinutes", defaults.snooze_minutes, ranges::SNOOZE),
            wallpaper_image_url_string: v
                .get("wallpaperImageURLString")
                .and_then(|x| x.as_str())
                .map(Settings::normalize_url_string)
                .unwrap_or_default(),
            wallpaper_theme: theme,
            launch_at_login: v
                .get("launchAtLogin")
                .and_then(|x| x.as_bool())
                .unwrap_or(defaults.launch_at_login),
            sound_enabled: v
                .get("soundEnabled")
                .and_then(|x| x.as_bool())
                .unwrap_or(defaults.sound_enabled),
            overlay_other_windows: v
                .get("overlayOtherWindows")
                .and_then(|x| x.as_bool())
                .unwrap_or(defaults.overlay_other_windows),
            activity_based_timing: v
                .get("activityBasedTiming")
                .and_then(|x| x.as_bool())
                .unwrap_or(defaults.activity_based_timing),
            idle_threshold_minutes: get_i(
                "idleThresholdMinutes",
                defaults.idle_threshold_minutes,
                ranges::IDLE_THRESHOLD,
            ),
            auto_start_break: v
                .get("autoStartBreak")
                .and_then(|x| x.as_bool())
                .unwrap_or(defaults.auto_start_break),
            auto_start_break_delay_seconds: get_i(
                "autoStartBreakDelaySeconds",
                defaults.auto_start_break_delay_seconds,
                ranges::AUTO_START_DELAY,
            ),
            reminder_window_opacity: opacity,
            app_language: lang,
            skipped_update_version: skipped,
        }
    }

    pub fn save(&self, s: &Settings) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(s)?;
        std::fs::write(&self.path, json)
    }
}

fn parse_theme(s: &str) -> Option<WallpaperTheme> {
    match s {
        "nature" => Some(WallpaperTheme::Nature),
        "cityNight" => Some(WallpaperTheme::CityNight),
        "minimal" => Some(WallpaperTheme::Minimal),
        "cartoon" => Some(WallpaperTheme::Cartoon),
        _ => None,
    }
}

/// 应用层"写设置即钳制"的入口：更新共享设置并返回钳制后的完整快照。
pub type SharedSettings = std::sync::Arc<std::sync::Mutex<Settings>>;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "ok")]
pub enum SetResult {
    Ok { value: serde_json::Value },
    UnknownKey,
    BadValue,
}

/// 单个键的写入路径：解析 → 钳制/归一化 → 写回 settings → 持久化回调由调用方处理。
pub fn apply_setting(s: &mut Settings, key: &str, value: &serde_json::Value) -> SetResult {
    macro_rules! set_bool {
        ($field:ident) => {
            match value.as_bool() {
                Some(b) => s.$field = b,
                None => return SetResult::BadValue,
            }
        };
    }
    macro_rules! set_int {
        ($field:ident, $range:expr) => {
            match value.as_i64() {
                Some(n) => s.$field = clamp_i(n, $range),
                None => return SetResult::BadValue,
            }
        };
    }
    match key {
        "reminderIntervalMinutes" => set_int!(reminder_interval_minutes, ranges::INTERVAL),
        "breakDurationMinutes" => set_int!(break_duration_minutes, ranges::BREAK_DURATION),
        "snoozeMinutes" => set_int!(snooze_minutes, ranges::SNOOZE),
        "idleThresholdMinutes" => set_int!(idle_threshold_minutes, ranges::IDLE_THRESHOLD),
        "autoStartBreakDelaySeconds" => {
            set_int!(auto_start_break_delay_seconds, ranges::AUTO_START_DELAY)
        }
        "reminderWindowOpacity" => {
            let Some(f) = value.as_f64() else {
                return SetResult::BadValue;
            };
            s.reminder_window_opacity =
                f.clamp(ranges::WINDOW_OPACITY.0, ranges::WINDOW_OPACITY.1);
        }
        "wallpaperImageURLString" => {
            let Some(t) = value.as_str() else {
                return SetResult::BadValue;
            };
            s.wallpaper_image_url_string = Settings::normalize_url_string(t);
        }
        "soundEnabled" => set_bool!(sound_enabled),
        "overlayOtherWindows" => set_bool!(overlay_other_windows),
        "activityBasedTiming" => set_bool!(activity_based_timing),
        "autoStartBreak" => set_bool!(auto_start_break),
        "wallpaperTheme" => {
            let Some(name) = value.as_str() else {
                return SetResult::BadValue;
            };
            match parse_theme(name) {
                Some(theme) => s.wallpaper_theme = theme,
                None => return SetResult::BadValue,
            }
        }
        _ => return SetResult::UnknownKey,
    }
    SetResult::Ok {
        value: serde_json::json!({
            "reminderIntervalMinutes": s.reminder_interval_minutes,
            "breakDurationMinutes": s.break_duration_minutes,
            "snoozeMinutes": s.snooze_minutes,
            "idleThresholdMinutes": s.idle_threshold_minutes,
            "autoStartBreakDelaySeconds": s.auto_start_break_delay_seconds,
            "reminderWindowOpacity": s.reminder_window_opacity,
            "wallpaperImageURLString": s.wallpaper_image_url_string,
            "wallpaperTheme": theme_name(s.wallpaper_theme),
            "soundEnabled": s.sound_enabled,
            "overlayOtherWindows": s.overlay_other_windows,
            "activityBasedTiming": s.activity_based_timing,
            "autoStartBreak": s.auto_start_break,
            "launchAtLogin": s.launch_at_login,
            "appLanguage": s.app_language,
        }),
    }
}

pub fn theme_name(t: WallpaperTheme) -> &'static str {
    match t {
        WallpaperTheme::Nature => "nature",
        WallpaperTheme::CityNight => "cityNight",
        WallpaperTheme::Minimal => "minimal",
        WallpaperTheme::Cartoon => "cartoon",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_settings_defaults() {
        let d = Settings::default();
        assert_eq!(d.reminder_interval_minutes, 45);
        assert_eq!(d.break_duration_minutes, 5);
        assert_eq!(d.snooze_minutes, 5);
        assert_eq!(d.wallpaper_image_url_string, "");
        assert_eq!(d.wallpaper_theme, WallpaperTheme::Nature);
        assert!(d.launch_at_login);
        assert!(d.sound_enabled);
        assert!(!d.overlay_other_windows);
        assert!(d.activity_based_timing);
        assert_eq!(d.idle_threshold_minutes, 2);
        assert!(d.auto_start_break);
        assert_eq!(d.auto_start_break_delay_seconds, 0);
        assert_eq!(d.reminder_window_opacity, 1.0);
        assert_eq!(d.app_language, "chinese");
    }

    #[test]
    fn test_load_invalid_values_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("pause-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = SettingsFile::new(&dir);
        std::fs::write(
            file.path.clone(),
            serde_json::json!({
                "reminderIntervalMinutes": 5,       // 超范围 → 回退 45（不是钳到 10）
                "breakDurationMinutes": 999,        // → 回退 5
                "reminderWindowOpacity": 0.05,      // → 回退 1.0
                "wallpaperImageURLString": "bad url",
                "appLanguage": "chinese"
            })
            .to_string(),
        )
        .unwrap();
        let s = file.load();
        assert_eq!(s.reminder_interval_minutes, 45);
        assert_eq!(s.break_duration_minutes, 5);
        assert_eq!(s.reminder_window_opacity, 1.0);
        assert_eq!(s.wallpaper_image_url_string, "");
        assert_eq!(s.app_language, "chinese");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_wallpaper_url_validation() {
        for good in [
            "https://images.example.com/photo.jpg",
            "http://example.com/a.png",
        ] {
            assert!(Settings::is_http_url(good), "{good} should be valid");
        }
        for bad in [
            "",
            "   ",
            "ftp://example.com/x",
            "images.example.com/photo.jpg",
            "not a url",
        ] {
            assert!(!Settings::is_http_url(bad), "{bad} should be invalid");
        }
        // 默认 picsum 地址归一化为空串（兼容旧版持久化过的默认值）
        assert_eq!(
            Settings::normalize_url_string(DEFAULT_WALLPAPER_URL),
            ""
        );
        assert_eq!(
            Settings::normalize_url_string("  https://images.example.com/p.jpg  "),
            "https://images.example.com/p.jpg"
        );
        assert_eq!(Settings::normalize_url_string("bad url"), "");
    }

    #[test]
    fn test_apply_setting_clamps_on_write() {
        let mut s = Settings::default();
        // 写入时是钳制不是回退：interval=5 → 10
        match apply_setting(&mut s, "reminderIntervalMinutes", &serde_json::json!(5)) {
            SetResult::Ok { value } => {
                assert_eq!(value["reminderIntervalMinutes"], 10)
            }
            _ => panic!("expected ok"),
        }
        apply_setting(&mut s, "reminderWindowOpacity", &serde_json::json!(2.0));
        assert_eq!(s.reminder_window_opacity, 1.0);
        apply_setting(&mut s, "snoozeMinutes", &serde_json::json!(99));
        assert_eq!(s.snooze_minutes, 15);
        // 自动倒计时：0 合法（立即休息）；负数钳到 0；超上限钳到 300
        apply_setting(&mut s, "autoStartBreakDelaySeconds", &serde_json::json!(0));
        assert_eq!(s.auto_start_break_delay_seconds, 0);
        apply_setting(&mut s, "autoStartBreakDelaySeconds", &serde_json::json!(-3));
        assert_eq!(s.auto_start_break_delay_seconds, 0);
        apply_setting(&mut s, "autoStartBreakDelaySeconds", &serde_json::json!(999));
        assert_eq!(s.auto_start_break_delay_seconds, 300);
        assert!(matches!(
            apply_setting(&mut s, "noSuchKey", &serde_json::json!(1)),
            SetResult::UnknownKey
        ));
    }
}
