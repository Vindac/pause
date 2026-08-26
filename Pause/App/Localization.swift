import Combine
import Foundation

/// 应用界面语言。
enum AppLanguage: String, CaseIterable, Identifiable {
    case english
    case chinese

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .english: return "English"
        case .chinese: return "简体中文"
        }
    }
}

/// 全部界面文案的键。中英文案以代码内建表维护：
/// SPM 可执行目标无 bundle 本地化资源，内建表才能支持应用内运行时切换。
enum L10nKey {
    // 通用
    case appName
    case settingsTitle

    // 设置页
    case sectionGeneral
    case languageLabel
    case versionLabel
    case sectionReminder
    case intervalLabel
    case minutes(Int)
    case customMinutes(Int)
    case customIntervalText(Int)
    case breakDurationText(Int)
    case snoozeDurationLabel
    case snoozeCustomPrefix
    case snoozeCustomUnit
    case snoozeCaption
    case usageTimingLabel
    case usageTimingHint
    case idleThresholdLabel
    case autoStartBreakLabel
    case autoStartBreakDelayLabel
    case autoStartBreakHint
    case seconds(Int)
    case sectionWallpaper
    case switchWallpaper
    case sectionSystem
    case launchAtLoginLabel
    case soundLabel
    case overlayLabel
    case sectionWindow
    case windowOpacityLabel
    case windowOpacityHint

    // 菜单栏
    case menuBreakNow
    case menuPause
    case menuResume
    case menuSettings
    case menuQuit
    case statusWaiting
    case statusIdle
    case statusNextBreak(Int)
    case statusSnoozed(Int)
    case statusReminding
    case statusBreaking(Int)
    case statusBreakingShort
    case statusPaused

    // 提醒页
    case reminderTitle
    case reminderSubtitle
    case reminderBreakFor(String)
    case reminderDelay(Int)
    case reminderStart
    case reminderStartIn(Int)

    // 休息页
    case breakTitle
    case breakHint
    case breakAlmostDone
    case breakSkip
}

extension L10nKey {

    func text(for language: AppLanguage) -> String {
        switch language {
        case .english: return en
        case .chinese: return zh
        }
    }

    private var en: String {
        switch self {
        case .appName: return "Pause"
        case .settingsTitle: return "Settings"

        case .sectionGeneral: return "General"
        case .languageLabel: return "Language"
        case .versionLabel: return "Version"
        case .sectionReminder: return "Reminders"
        case .intervalLabel: return "Remind every"
        case .minutes(let m): return "\(m) min"
        case .customMinutes(let m): return "\(m) min (custom)"
        case .customIntervalText(let m): return "Custom interval: \(m) min"
        case .breakDurationText(let m): return "Break duration: \(m) min"
        case .snoozeDurationLabel: return "Delay break for"
        case .snoozeCustomPrefix: return "Custom"
        case .snoozeCustomUnit: return "min"
        case .snoozeCaption:
            return "\"Delay break\" postpones the reminder when it pops up."
        case .usageTimingLabel: return "Count actual usage time"
        case .usageTimingHint:
            return "The timer counts only while you're actively using the Mac " +
                   "(keyboard/trackpad input). It pauses while you're away, the display sleeps, " +
                   "or the system sleeps — so you get reminded only after real usage fills the interval."
        case .idleThresholdLabel: return "Consider away after"
        case .autoStartBreakLabel: return "Auto-start break"
        case .autoStartBreakDelayLabel: return "Countdown"
        case .autoStartBreakHint:
            return "When a reminder pops up, a countdown starts; if you do nothing, " +
                   "the break begins automatically. You can still delay or start it manually."
        case .seconds(let s): return "\(s)s"
        case .sectionWallpaper: return "Wallpaper"
        case .switchWallpaper: return "Switch Image"
        case .sectionSystem: return "System"
        case .launchAtLoginLabel: return "Launch at login"
        case .soundLabel: return "Play a gentle sound on reminders"
        case .overlayLabel: return "Reminders overlay other windows"
        case .sectionWindow: return "Reminder Window"
        case .windowOpacityLabel: return "Window opacity"
        case .windowOpacityHint:
            return "Lower values make the window more translucent. " +
                   "Changes apply immediately while a reminder is showing."

        case .menuBreakNow: return "Break Now"
        case .menuPause: return "Pause Reminders"
        case .menuResume: return "Resume Reminders"
        case .menuSettings: return "Settings…"
        case .menuQuit: return "Quit Pause"
        case .statusWaiting: return "Break is due — waiting for the right moment…"
        case .statusIdle: return "No activity detected — timer paused"
        case .statusNextBreak(let m): return "Next break in \(m) min"
        case .statusSnoozed(let m): return "Delayed — reminds again in \(m) min"
        case .statusReminding: return "Time for a break"
        case .statusBreaking(let m): return "On break · \(m) min left"
        case .statusBreakingShort: return "On break"
        case .statusPaused: return "Reminders paused"

        case .reminderTitle: return "Time to rest your eyes"
        case .reminderSubtitle: return "Look into the distance and stretch a little"
        case .reminderBreakFor(let t): return "Break for \(t)"
        case .reminderDelay(let m): return "Delay \(m) min"
        case .reminderStart: return "Start Break"
        case .reminderStartIn(let s): return "Start Break (\(s)s)"

        case .breakTitle: return "Look out the window"
        case .breakHint: return "Stand up and move around"
        case .breakAlmostDone: return "Break over — welcome back"
        case .breakSkip: return "End Early"
        }
    }

    private var zh: String {
        switch self {
        case .appName: return "休一下"
        case .settingsTitle: return "设置"

        case .sectionGeneral: return "通用"
        case .languageLabel: return "语言"
        case .versionLabel: return "版本"
        case .sectionReminder: return "提醒"
        case .intervalLabel: return "每隔"
        case .minutes(let m): return "\(m) 分钟"
        case .customMinutes(let m): return "\(m) 分钟（自定义）"
        case .customIntervalText(let m): return "自定义间隔：\(m) 分钟"
        case .breakDurationText(let m): return "休息时长：\(m) 分钟"
        case .snoozeDurationLabel: return "延迟休息时间"
        case .snoozeCustomPrefix: return "自定义"
        case .snoozeCustomUnit: return "分钟"
        case .snoozeCaption:
            return "提醒弹出后点「延迟休息」可推迟下一次提醒。"
        case .usageTimingLabel: return "按真实使用时间计时"
        case .usageTimingHint:
            return "开启后仅在有键鼠使用时累计倒计时；离开电脑、显示器睡眠或系统睡眠期间自动暂停，" +
                   "累计真实使用满间隔才提醒，而不是固定时间一到就提醒。"
        case .idleThresholdLabel: return "离开判定"
        case .autoStartBreakLabel: return "自动开始休息"
        case .autoStartBreakDelayLabel: return "自动倒计时"
        case .autoStartBreakHint:
            return "提醒弹出后开始倒计时，期间无操作则自动进入休息；随时仍可点「延迟休息」或「开始休息」。"
        case .seconds(let s): return "\(s) 秒"
        case .sectionWallpaper: return "图片"
        case .switchWallpaper: return "切换图片"
        case .sectionSystem: return "系统"
        case .launchAtLoginLabel: return "开机自动启动"
        case .soundLabel: return "提醒时播放轻提示音"
        case .overlayLabel: return "提醒时覆盖其他窗口"
        case .sectionWindow: return "提醒窗口"
        case .windowOpacityLabel: return "窗口透明度"
        case .windowOpacityHint:
            return "数值越低窗口越通透，可隐约看到桌面；提醒窗口显示时修改立即生效。"

        case .menuBreakNow: return "立即休息"
        case .menuPause: return "暂停提醒"
        case .menuResume: return "继续提醒"
        case .menuSettings: return "设置…"
        case .menuQuit: return "退出 休一下"
        case .statusWaiting: return "休息时间到，等待合适时机弹出…"
        case .statusIdle: return "未检测到使用 · 计时已暂停"
        case .statusNextBreak(let m): return "下次休息：\(m) 分钟后"
        case .statusSnoozed(let m): return "已延迟休息：\(m) 分钟后再次提醒"
        case .statusReminding: return "该休息一下了"
        case .statusBreaking(let m): return "休息中 · 剩余 \(m) 分钟"
        case .statusBreakingShort: return "休息中"
        case .statusPaused: return "提醒已暂停"

        case .reminderTitle: return "该让眼睛休息一下了"
        case .reminderSubtitle: return "看看远处，活动一下身体"
        case .reminderBreakFor(let t): return "休息 \(t)"
        case .reminderDelay(let m): return "延迟休息 \(m) 分钟"
        case .reminderStart: return "开始休息"
        case .reminderStartIn(let s): return "开始休息（\(s) 秒）"

        case .breakTitle: return "看看窗外吧"
        case .breakHint: return "站起来活动一下"
        case .breakAlmostDone: return "休息结束，欢迎回来"
        case .breakSkip: return "提前结束"
        }
    }
}

/// 语言状态（Service）：读写 UserDefaults，@Published 驱动全界面即时切换。
@MainActor
final class LocalizationStore: ObservableObject {

    enum Key {
        static let appLanguage = "appLanguage"
    }

    @Published private(set) var language: AppLanguage

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        let raw = defaults.string(forKey: Key.appLanguage) ?? ""
        self.language = AppLanguage(rawValue: raw) ?? .english   // 默认英文
    }

    func setLanguage(_ language: AppLanguage) {
        guard language != self.language else { return }
        self.language = language
        defaults.set(language.rawValue, forKey: Key.appLanguage)
    }

    func t(_ key: L10nKey) -> String {
        key.text(for: language)
    }
}
