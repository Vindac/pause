import AppKit
import Combine
import Foundation

/// 设置页 ViewModel：绑定 SettingsStore，所有修改立即保存（无"保存"按钮）。
/// 开机启动项额外同步 SMAppService；壁纸区提供预览与手动切换。
@MainActor
final class SettingsViewModel: ObservableObject {

    @Published var reminderIntervalMinutes: Int { didSet { store.reminderIntervalMinutes = reminderIntervalMinutes } }
    @Published var breakDurationMinutes: Int { didSet { store.breakDurationMinutes = breakDurationMinutes } }
    @Published var snoozeMinutes: Int { didSet { store.snoozeMinutes = snoozeMinutes } }
    /// 延迟时间的自定义输入框文本（解析成功即写入 snoozeMinutes）
    @Published var customSnoozeText: String = ""
    @Published var launchAtLogin: Bool {
        didSet {
            store.launchAtLogin = launchAtLogin
            if launchAtLoginService.setEnabled(launchAtLogin) == false {
                // 注册失败（例如裸二进制运行）时回退 UI 状态，保持显示与事实一致
                if launchAtLoginService.isEnabled != launchAtLogin {
                    launchAtLogin = launchAtLoginService.isEnabled
                }
            }
        }
    }
    @Published var soundEnabled: Bool { didSet { store.soundEnabled = soundEnabled } }
    @Published var overlayOtherWindows: Bool { didSet { store.overlayOtherWindows = overlayOtherWindows } }
    @Published var activityBasedTiming: Bool { didSet { store.activityBasedTiming = activityBasedTiming } }
    @Published var idleThresholdMinutes: Int { didSet { store.idleThresholdMinutes = idleThresholdMinutes } }
    @Published var autoStartBreak: Bool { didSet { store.autoStartBreak = autoStartBreak } }
    @Published var autoStartBreakDelaySeconds: Int { didSet { store.autoStartBreakDelaySeconds = autoStartBreakDelaySeconds } }
    @Published var reminderWindowOpacity: Double { didSet { store.reminderWindowOpacity = reminderWindowOpacity } }

    /// 壁纸预览（跟随 WallpaperService 当前图）
    @Published private(set) var wallpaperPreview: NSImage?

    /// 快捷选项（30/45/60），自定义值时 Picker 显示"自定义"
    static let quickIntervals = [30, 45, 60]
    /// 延迟休息快捷选项 1–5 分钟，其余视为自定义
    static let quickSnoozeMinutes = [1, 2, 3, 4, 5]
    /// 离开判定快捷选项（分钟）
    static let quickIdleThresholds = [1, 2, 3, 5]
    /// 自动开始休息倒计时快捷选项（秒）
    static let quickAutoBreakDelays = [10, 20, 30, 60]

    /// 透明度显示为百分比
    var opacityPercentText: String {
        "\(Int((reminderWindowOpacity * 100).rounded()))%"
    }

    private let store: SettingsStore
    private let launchAtLoginService: LaunchAtLoginService
    private let wallpapers: WallpaperService
    private let localization: LocalizationStore
    private var cancellables: Set<AnyCancellable> = []

    init(store: SettingsStore,
         launchAtLoginService: LaunchAtLoginService,
         wallpapers: WallpaperService,
         localization: LocalizationStore) {
        self.store = store
        self.launchAtLoginService = launchAtLoginService
        self.wallpapers = wallpapers
        self.localization = localization
        reminderIntervalMinutes = store.reminderIntervalMinutes
        breakDurationMinutes = store.breakDurationMinutes
        snoozeMinutes = store.snoozeMinutes
        customSnoozeText = "\(store.snoozeMinutes)"
        soundEnabled = store.soundEnabled
        overlayOtherWindows = store.overlayOtherWindows
        activityBasedTiming = store.activityBasedTiming
        idleThresholdMinutes = store.idleThresholdMinutes
        autoStartBreak = store.autoStartBreak
        autoStartBreakDelaySeconds = store.autoStartBreakDelaySeconds
        reminderWindowOpacity = store.reminderWindowOpacity
        launchAtLogin = store.launchAtLogin || launchAtLoginService.isEnabled

        wallpapers.$current
            .receive(on: RunLoop.main)
            .sink { [weak self] item in self?.wallpaperPreview = item?.image }
            .store(in: &cancellables)
    }

    var isCustomInterval: Bool {
        !Self.quickIntervals.contains(reminderIntervalMinutes)
    }

    var isCustomSnooze: Bool {
        !Self.quickSnoozeMinutes.contains(snoozeMinutes)
    }

    // MARK: - 延迟休息时间

    /// Picker 选择快捷值时同步自定义输入框
    func selectSnoozeMinutes(_ minutes: Int) {
        snoozeMinutes = minutes
        customSnoozeText = "\(minutes)"
    }

    /// 自定义输入：解析为 1–15 分钟并写入（输入过程中的不完整文本被忽略）
    func commitCustomSnoozeText() {
        let trimmed = customSnoozeText.trimmingCharacters(in: .whitespaces)
        guard let value = Int(trimmed) else { return }
        let clamped = min(max(value, ReminderSettings.snoozeRange.lowerBound),
                          ReminderSettings.snoozeRange.upperBound)
        snoozeMinutes = clamped
        customSnoozeText = "\(clamped)"
    }

    // MARK: - 语言

    var language: AppLanguage {
        localization.language
    }

    func setLanguage(_ language: AppLanguage) {
        localization.setLanguage(language)
    }

    // MARK: - 壁纸

    /// 立即切换到下一张壁纸（预取图或缓存图），并触发新的预取
    func switchWallpaper() {
        wallpapers.advance()
    }
}
