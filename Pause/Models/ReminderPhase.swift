import Foundation

/// 提醒状态机的全部阶段。
/// 纯数据模型，不依赖 UI 与系统服务。
enum ReminderPhase: Equatable {
    /// 工作计时中，deadline 到达后触发提醒
    case working(deadline: Date)
    /// 用户选择了"稍后提醒"
    case snoozing(deadline: Date, snoozeCount: Int)
    /// 提醒窗口展示中，等待用户操作。
    /// autoBreakAt：开启"自动开始休息"时的自动进入休息截止时刻（nil = 纯手动）
    case reminding(autoBreakAt: Date?)
    /// 休息倒计时进行中
    case breaking(BreakSession)
    /// 用户手动暂停全部提醒
    case paused

    /// 阶段是否处于"等待下一个时间点"（工作 / 稍后）
    var pendingDeadline: Date? {
        switch self {
        case .working(let deadline): return deadline
        case .snoozing(let deadline, _): return deadline
        default: return nil
        }
    }
}
