import Combine
import Foundation

/// 设置的统一读写入口（Service）。
/// View 不直接接触 UserDefaults；ViewModel 通过本类读写，修改立即持久化。
@MainActor
final class SettingsStore: ObservableObject {

    /// UserDefaults 键名（与设计文档第 9 节一致）
    enum Key {
        static let reminderIntervalMinutes = "reminderIntervalMinutes"
        static let breakDurationMinutes = "breakDurationMinutes"
        static let snoozeMinutes = "snoozeMinutes"
        static let wallpaperImageURLString = "wallpaperImageURLString"
        static let wallpaperTheme = "wallpaperTheme"
        static let launchAtLogin = "launchAtLogin"
        static let soundEnabled = "soundEnabled"
        static let overlayOtherWindows = "overlayOtherWindows"
        static let activityBasedTiming = "activityBasedTiming"
        static let idleThresholdMinutes = "idleThresholdMinutes"
        static let autoStartBreak = "autoStartBreak"
        static let autoStartBreakDelaySeconds = "autoStartBreakDelaySeconds"
        static let reminderWindowOpacity = "reminderWindowOpacity"
    }

    private let defaults: UserDefaults

    @Published var reminderIntervalMinutes: Int {
        didSet {
            defaults.set(min(max(reminderIntervalMinutes, ReminderSettings.intervalRange.lowerBound),
                             ReminderSettings.intervalRange.upperBound),
                         forKey: Key.reminderIntervalMinutes)
        }
    }

    @Published var breakDurationMinutes: Int {
        didSet {
            defaults.set(min(max(breakDurationMinutes, ReminderSettings.breakDurationRange.lowerBound),
                             ReminderSettings.breakDurationRange.upperBound),
                         forKey: Key.breakDurationMinutes)
        }
    }

    @Published var snoozeMinutes: Int {
        didSet {
            defaults.set(min(max(snoozeMinutes, ReminderSettings.snoozeRange.lowerBound),
                             ReminderSettings.snoozeRange.upperBound),
                         forKey: Key.snoozeMinutes)
        }
    }

    /// 在线取图地址：去掉首尾空白后持久化
    @Published var wallpaperImageURLString: String {
        didSet {
            let trimmed = wallpaperImageURLString.trimmingCharacters(in: .whitespacesAndNewlines)
            if trimmed != wallpaperImageURLString { wallpaperImageURLString = trimmed }
            defaults.set(trimmed, forKey: Key.wallpaperImageURLString)
        }
    }

    @Published var wallpaperTheme: WallpaperTheme {
        didSet { defaults.set(wallpaperTheme.rawValue, forKey: Key.wallpaperTheme) }
    }

    @Published var launchAtLogin: Bool {
        didSet { defaults.set(launchAtLogin, forKey: Key.launchAtLogin) }
    }

    @Published var soundEnabled: Bool {
        didSet { defaults.set(soundEnabled, forKey: Key.soundEnabled) }
    }

    @Published var overlayOtherWindows: Bool {
        didSet { defaults.set(overlayOtherWindows, forKey: Key.overlayOtherWindows) }
    }

    @Published var activityBasedTiming: Bool {
        didSet { defaults.set(activityBasedTiming, forKey: Key.activityBasedTiming) }
    }

    @Published var idleThresholdMinutes: Int {
        didSet {
            defaults.set(min(max(idleThresholdMinutes, ReminderSettings.idleThresholdRange.lowerBound),
                             ReminderSettings.idleThresholdRange.upperBound),
                         forKey: Key.idleThresholdMinutes)
        }
    }

    @Published var autoStartBreak: Bool {
        didSet { defaults.set(autoStartBreak, forKey: Key.autoStartBreak) }
    }

    @Published var autoStartBreakDelaySeconds: Int {
        didSet {
            defaults.set(min(max(autoStartBreakDelaySeconds, ReminderSettings.autoStartBreakDelayRange.lowerBound),
                             ReminderSettings.autoStartBreakDelayRange.upperBound),
                         forKey: Key.autoStartBreakDelaySeconds)
        }
    }

    @Published var reminderWindowOpacity: Double {
        didSet {
            let clamped = min(max(reminderWindowOpacity, ReminderSettings.windowOpacityRange.lowerBound),
                              ReminderSettings.windowOpacityRange.upperBound)
            if clamped != reminderWindowOpacity { reminderWindowOpacity = clamped }
            defaults.set(reminderWindowOpacity, forKey: Key.reminderWindowOpacity)
        }
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults

        let settings = Self.load(from: defaults)
        reminderIntervalMinutes = settings.reminderIntervalMinutes
        breakDurationMinutes = settings.breakDurationMinutes
        snoozeMinutes = settings.snoozeMinutes
        wallpaperImageURLString = settings.wallpaperImageURLString
        wallpaperTheme = settings.wallpaperTheme
        launchAtLogin = settings.launchAtLogin
        soundEnabled = settings.soundEnabled
        overlayOtherWindows = settings.overlayOtherWindows
        activityBasedTiming = settings.activityBasedTiming
        idleThresholdMinutes = settings.idleThresholdMinutes
        autoStartBreak = settings.autoStartBreak
        autoStartBreakDelaySeconds = settings.autoStartBreakDelaySeconds
        reminderWindowOpacity = settings.reminderWindowOpacity
    }

    /// 纯函数：从 UserDefaults 读取并应用默认值与合法范围（可测试）
    nonisolated static func load(from defaults: UserDefaults) -> ReminderSettings {
        var settings = ReminderSettings()
        settings.reminderIntervalMinutes = clamp(
            defaults.object(forKey: Key.reminderIntervalMinutes) as? Int ?? 45,
            ReminderSettings.intervalRange, fallback: 45)
        settings.breakDurationMinutes = clamp(
            defaults.object(forKey: Key.breakDurationMinutes) as? Int ?? 5,
            ReminderSettings.breakDurationRange, fallback: 5)
        settings.snoozeMinutes = clamp(
            defaults.object(forKey: Key.snoozeMinutes) as? Int ?? 5,
            ReminderSettings.snoozeRange, fallback: 5)
        let storedURL = defaults.string(forKey: Key.wallpaperImageURLString)?
            .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        // 留空 = 默认图片服务；仅用户输入的合法且不同于默认服务的地址被保留
        // （旧版本曾把预填的默认地址持久化，这里归一化为空）
        settings.wallpaperImageURLString =
            (Self.isHTTPURL(storedURL) && storedURL != ReminderSettings.defaultWallpaperImageURLString)
            ? storedURL : ""
        settings.wallpaperTheme = WallpaperTheme(
            rawValue: defaults.string(forKey: Key.wallpaperTheme) ?? "") ?? .nature
        settings.launchAtLogin = defaults.object(forKey: Key.launchAtLogin) as? Bool ?? true
        settings.soundEnabled = defaults.object(forKey: Key.soundEnabled) as? Bool ?? true
        settings.overlayOtherWindows = defaults.object(forKey: Key.overlayOtherWindows) as? Bool ?? false
        settings.activityBasedTiming = defaults.object(forKey: Key.activityBasedTiming) as? Bool ?? true
        settings.idleThresholdMinutes = clamp(
            defaults.object(forKey: Key.idleThresholdMinutes) as? Int ?? 2,
            ReminderSettings.idleThresholdRange, fallback: 2)
        settings.autoStartBreak = defaults.object(forKey: Key.autoStartBreak) as? Bool ?? true
        settings.autoStartBreakDelaySeconds = clamp(
            defaults.object(forKey: Key.autoStartBreakDelaySeconds) as? Int ?? 30,
            ReminderSettings.autoStartBreakDelayRange, fallback: 30)
        let storedOpacity = defaults.object(forKey: Key.reminderWindowOpacity) as? Double ?? 1.0
        settings.reminderWindowOpacity = ReminderSettings.windowOpacityRange.contains(storedOpacity) ? storedOpacity : 1.0
        return settings
    }

    /// 地址是否为可用的 http(s) URL
    nonisolated static func isHTTPURL(_ string: String) -> Bool {
        guard let url = URL(string: string),
              let scheme = url.scheme?.lowercased(),
              ["http", "https"].contains(scheme),
              url.host != nil else { return false }
        return true
    }

    /// 当前地址合法时构造 URL；空或非法返回 nil（Provider 回退默认图片服务）
    var wallpaperImageURL: URL? {
        let s = wallpaperImageURLString.trimmingCharacters(in: .whitespacesAndNewlines)
        guard Self.isHTTPURL(s) else { return nil }
        return URL(string: s)
    }

    nonisolated private static func clamp(_ value: Int, _ range: ClosedRange<Int>, fallback: Int) -> Int {
        range.contains(value) ? value : fallback
    }

    var current: ReminderSettings {
        ReminderSettings(
            reminderIntervalMinutes: reminderIntervalMinutes,
            breakDurationMinutes: breakDurationMinutes,
            snoozeMinutes: snoozeMinutes,
            wallpaperImageURLString: wallpaperImageURLString,
            wallpaperTheme: wallpaperTheme,
            launchAtLogin: launchAtLogin,
            soundEnabled: soundEnabled,
            overlayOtherWindows: overlayOtherWindows,
            activityBasedTiming: activityBasedTiming,
            idleThresholdMinutes: idleThresholdMinutes,
            autoStartBreak: autoStartBreak,
            autoStartBreakDelaySeconds: autoStartBreakDelaySeconds,
            reminderWindowOpacity: reminderWindowOpacity
        )
    }
}
