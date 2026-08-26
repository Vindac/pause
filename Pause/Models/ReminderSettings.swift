import Foundation

/// 图片主题（仅用于无网络且无缓存时的运行时兜底渲染）
enum WallpaperTheme: String, CaseIterable, Identifiable {
    case nature      // 自然风景
    case cityNight   // 城市夜景
    case minimal     // 极简渐变
    case cartoon     // 卡通插画

    var id: String { rawValue }
}

/// 纯数据模型：提醒相关全部设置（默认值与设计文档第 9 节一致）
struct ReminderSettings: Equatable {
    var reminderIntervalMinutes: Int = 45
    var breakDurationMinutes: Int = 5
    var snoozeMinutes: Int = 5
    /// 在线取图地址：空 = 使用默认图片服务（picsum.photos 随机图）；
    /// 可通过 defaults write 配置合法 http(s) 地址后按该地址取图
    var wallpaperImageURLString: String = ""
    var wallpaperTheme: WallpaperTheme = .nature
    var launchAtLogin: Bool = true
    var soundEnabled: Bool = true
    var overlayOtherWindows: Bool = false
    /// 按真实使用时间计时：无键鼠输入（离开/睡眠）期间暂停工作计时
    var activityBasedTiming: Bool = true
    /// 无输入多久判定为「离开」（分钟）
    var idleThresholdMinutes: Int = 2
    /// 提醒弹出后自动开始休息（无操作倒计时结束自动进入休息；关闭则纯手动）
    var autoStartBreak: Bool = true
    /// 自动开始休息的倒计时秒数
    var autoStartBreakDelaySeconds: Int = 30
    /// 提醒窗口整体不透明度（0.3–1.0，1.0 = 完全不透明）
    var reminderWindowOpacity: Double = 1.0

    static let defaultWallpaperImageURLString = "https://picsum.photos/2880/1800"

    /// 合法范围约束（10–180 分钟）
    static let intervalRange = 10...180
    static let breakDurationRange = 1...30
    static let snoozeRange = 1...15
    static let idleThresholdRange = 1...10
    static let autoStartBreakDelayRange = 5...300
    static let windowOpacityRange: ClosedRange<Double> = 0.3...1.0
}
