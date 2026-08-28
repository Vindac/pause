//! Localization.swift 的 Rust 移植：52 条中英内建文案。
//!
//! 文案表是**单一事实来源**：托盘菜单（Rust）与前端 UI 都从这里取词。
//! 带参数的 key 使用 `{name}` 占位符，`tf()` 与前端共享同一替换语义，
//! 保证两端渲染一致；语言切换时通过事件把全量字典推给前端即时生效。

use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Lang {
    English,
    Chinese,
}

impl Lang {
    pub fn parse(s: &str) -> Self {
        match s {
            "english" => Self::English,
            _ => Self::Chinese, // 默认中文：读不到或解析失败一律回退
        }
    }
    pub fn storage_key(self) -> &'static str {
        match self {
            Lang::English => "english",
            Lang::Chinese => "chinese",
        }
    }
}

/// (中文, 英文)。带 {placeholder} 的是模板项。
pub const STRINGS: &[(&str, (&str, &str))] = &[
    // 通用
    ("appName", ("休一下", "Pause")),
    ("settingsTitle", ("设置", "Settings")),
    // 设置页：通用
    ("sectionGeneral", ("通用", "General")),
    ("languageLabel", ("语言", "Language")),
    ("versionLabel", ("版本", "Version")),
    // 设置页：提醒
    ("sectionReminder", ("提醒", "Reminders")),
    ("intervalLabel", ("每隔", "Remind every")),
    ("minutes", ("{m} 分钟", "{m} min")),
    (
        "customMinutes",
        ("{m} 分钟（自定义）", "{m} min (custom)"),
    ),
    (
        "customIntervalText",
        ("自定义间隔：{m} 分钟", "Custom interval: {m} min"),
    ),
    ("breakDurationLabel", ("休息时长", "Break duration")),
    (
        "breakDurationText",
        ("休息时长：{m} 分钟", "Break duration: {m} min"),
    ),
    ("snoozeDurationLabel", ("延迟休息时间", "Delay break for")),
    ("snoozeCustomPrefix", ("自定义", "Custom")),
    ("snoozeCustomUnit", ("分钟", "min")),
    (
        "snoozeCaption",
        (
            "提醒弹出后点「延迟休息」可推迟下一次提醒。",
            "\"Delay break\" postpones the reminder when it pops up.",
        ),
    ),
    ("usageTimingLabel", ("按真实使用时间计时", "Count actual usage time")),
    (
        "usageTimingHint",
        (
            "开启后仅在有键鼠使用时累计倒计时；离开电脑、显示器睡眠或系统睡眠期间自动暂停，累计真实使用满间隔才提醒，而不是固定时间一到就提醒。",
            "The timer counts only while you're actively using the computer (keyboard/mouse input). It pauses while you're away, the display sleeps, or the system sleeps — so you get reminded only after real usage fills the interval.",
        ),
    ),
    ("idleThresholdLabel", ("离开判定", "Consider away after")),
    ("autoStartBreakLabel", ("自动开始休息", "Auto-start break")),
    ("autoStartBreakDelayLabel", ("自动倒计时", "Countdown")),
    ("noCountdown", ("立即", "Immediately")),
    (
        "autoStartBreakHint",
        (
            "提醒弹出后开始倒计时，期间无操作则自动进入休息；随时仍可点「延迟休息」或「开始休息」。",
            "When a reminder pops up, a countdown starts; if you do nothing, the break begins automatically. You can still delay or start it manually.",
        ),
    ),
    ("seconds", ("{s} 秒", "{s}s")),
    // 设置页：图片
    ("sectionWallpaper", ("图片", "Wallpaper")),
    ("switchWallpaper", ("切换图片", "Switch Image")),
    (
        "switchWallpaperLoading",
        ("获取新图中…", "Fetching new image…"),
    ),
    // 设置页：系统
    ("sectionSystem", ("系统", "System")),
    ("launchAtLoginLabel", ("开机自动启动", "Launch at login")),
    ("soundLabel", ("提醒时播放轻提示音", "Play a gentle sound on reminders")),
    (
        "overlayLabel",
        ("提醒时覆盖其他窗口", "Reminders overlay other windows"),
    ),
    // 设置页：检查更新（双通道：应用内更新 / GitHub 下载）
    ("checkUpdate", ("检查更新", "Check for Updates")),
    ("updateChecking", ("正在检查…", "Checking…")),
    ("upToDate", ("已是最新版本", "You're up to date")),
    (
        "updateCheckFailed",
        ("检查更新失败，请稍后重试", "Update check failed, try again later"),
    ),
    ("updateAvailable", ("发现新版本 {v}", "New version {v} available")),
    ("updateNow", ("立即更新", "Update Now")),
    ("updateFromGithub", ("前往 GitHub 下载", "Get it from GitHub")),
    (
        "updateGithubOnlyHint",
        ("此版本暂不支持应用内更新，请从 GitHub 下载安装。", "In-app update unavailable for this version — please download from GitHub."),
    ),
    (
        "updateDownloading",
        ("下载中… {p}%", "Downloading… {p}%"),
    ),
    (
        "updateInstalling",
        ("安装完成，即将重启…", "Installed — restarting…"),
    ),
    (
        "skipUpdateVersion",
        ("暂不提醒（本版本）", "Skip this version"),
    ),
    ("updateLater", ("稍后再说", "Later")),
    // 设置页：提醒窗口
    ("sectionWindow", ("提醒窗口", "Reminder Window")),
    ("windowOpacityLabel", ("窗口透明度", "Window opacity")),
    (
        "windowOpacityHint",
        (
            "数值越低窗口越通透，可隐约看到桌面；提醒窗口显示时修改立即生效。",
            "Lower values make the window more translucent. Changes apply immediately while a reminder is showing.",
        ),
    ),
    // 菜单栏
    ("menuBreakNow", ("立即休息", "Break Now")),
    ("menuPause", ("暂停提醒", "Pause Reminders")),
    ("menuResume", ("继续提醒", "Resume Reminders")),
    ("menuSettings", ("设置…", "Settings…")),
    ("menuQuit", ("退出 休一下", "Quit Pause")),
    (
        "statusWaiting",
        ("休息时间到，等待合适时机弹出…", "Break is due — waiting for the right moment…"),
    ),
    (
        "statusIdle",
        ("未检测到使用 · 计时已暂停", "No activity detected — timer paused"),
    ),
    (
        "statusNextBreak",
        ("下次休息：{m} 分钟后", "Next break in {m} min"),
    ),
    (
        "statusSnoozed",
        ("已延迟休息：{m} 分钟后再次提醒", "Delayed — reminds again in {m} min"),
    ),
    ("statusReminding", ("该休息一下了", "Time for a break")),
    (
        "statusBreaking",
        ("休息中 · 剩余 {m} 分钟", "On break · {m} min left"),
    ),
    ("statusBreakingShort", ("休息中", "On break")),
    ("statusPaused", ("提醒已暂停", "Reminders paused")),
    // 提醒页
    ("reminderTitle", ("该让眼睛休息一下了", "Time to rest your eyes")),
    (
        "reminderSubtitle",
        ("看看远处，活动一下身体", "Look into the distance and stretch a little"),
    ),
    ("reminderBreakFor", ("休息 {t}", "Break for {t}")),
    ("reminderDelay", ("延迟休息 {m} 分钟", "Delay {m} min")),
    ("reminderStart", ("开始休息", "Start Break")),
    (
        "reminderStartIn",
        ("开始休息（{s} 秒）", "Start Break ({s}s)"),
    ),
    // 休息页
    ("breakTitle", ("看看窗外吧", "Look out the window")),
    ("breakHint", ("站起来活动一下", "Stand up and move around")),
    ("breakAlmostDone", ("休息结束，欢迎回来", "Break over — welcome back")),
    ("breakSkip", ("提前结束", "End Early")),
];

/// 取静态/模板原文（含占位符，未替换参数）。
pub fn tr(lang: Lang, key: &str) -> String {
    for (k, (zh, en)) in STRINGS {
        if *k == key {
            return match lang {
                Lang::Chinese => (*zh).to_string(),
                Lang::English => (*en).to_string(),
            };
        }
    }
    key.to_string()
}

/// 模板格式化：{name} 替换。原版 `\(m)` 语义对齐。
pub fn tf(lang: Lang, key: &str, params: &[(&str, String)]) -> String {
    let mut out = tr(lang, key);
    for (name, value) in params {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

/// 全量字典：语言切换时整体推给前端，避免双端漂移。
pub fn strings_map(lang: Lang) -> BTreeMap<String, String> {
    STRINGS
        .iter()
        .map(|(k, (zh, en))| {
            let v = match lang {
                Lang::Chinese => *zh,
                Lang::English => *en,
            };
            ((*k).to_string(), v.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_dictionary_count() {
        // 内建表 69 条（原版 52 条基础上按同样 key 集合整理，多出者为
        // snooze 单位等边缘 key、切换图片加载态与检查更新双通道文案，
        // 菜单/前端共用同表故计数以本表为准）
        assert_eq!(STRINGS.len(), 69);
        // 所有 key 中英都非空
        for (k, (zh, en)) in STRINGS {
            assert!(!k.is_empty() && !zh.is_empty() && !en.is_empty());
        }
    }

    #[test]
    fn test_templates_format() {
        assert_eq!(tf(Lang::Chinese, "minutes", &[("m", "45".into())]), "45 分钟");
        assert_eq!(tf(Lang::English, "seconds", &[("s", "9".into())]), "9s");
        assert_eq!(
            tf(Lang::Chinese, "reminderStartIn", &[("s", "9".into())]),
            "开始休息（9 秒）"
        );
        assert_eq!(
            tf(Lang::English, "reminderStartIn", &[("s", "9".into())]),
            "Start Break (9s)"
        );
        assert_eq!(
            tf(Lang::English, "statusBreaking", &[("m", "3".into())]),
            "On break · 3 min left"
        );
        assert_eq!(
            tf(Lang::Chinese, "statusBreaking", &[("m", "3".into())]),
            "休息中 · 剩余 3 分钟"
        );
    }

    #[test]
    fn test_default_language_is_chinese() {
        assert_eq!(Lang::parse(""), Lang::Chinese);
        assert_eq!(Lang::parse("english"), Lang::English);
        assert_eq!(tr(Lang::English, "appName"), "Pause");
        assert_eq!(tr(Lang::Chinese, "appName"), "休一下");
        // 「无」改为「立即」：0 秒 = 弹出后直接开始休息
        assert_eq!(tr(Lang::Chinese, "noCountdown"), "立即");
        assert_eq!(tr(Lang::English, "noCountdown"), "Immediately");
    }

    #[test]
    fn test_maps_cover_all_keys() {
        let m = strings_map(Lang::Chinese);
        assert_eq!(m.len(), 69);
        assert!(m.contains_key("noCountdown"));
        assert!(m.contains_key("windowOpacityHint"));
        assert!(m.contains_key("switchWallpaperLoading"));
        assert!(m.contains_key("updateNow"));
        assert!(m.contains_key("updateFromGithub"));
    }
}
