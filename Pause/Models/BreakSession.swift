import Foundation

/// 一次休息会话的纯数据描述。
struct BreakSession: Equatable {
    let startedAt: Date
    let duration: TimeInterval

    init(startedAt: Date = Date(), duration: TimeInterval) {
        self.startedAt = startedAt
        self.duration = duration
    }

    var endsAt: Date { startedAt.addingTimeInterval(duration) }

    func remaining(at now: Date = Date()) -> TimeInterval {
        max(0, endsAt.timeIntervalSince(now))
    }

    /// mm:ss 格式（倒计时显示用）
    static func format(_ remaining: TimeInterval) -> String {
        let total = Int(remaining.rounded())
        let m = total / 60
        let s = total % 60
        return String(format: "%02d:%02d", m, s)
    }
}
