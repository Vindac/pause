import Combine
import Foundation

/// 休息倒计时页 ViewModel。
/// 常驻订阅提醒状态机的 breaking 会话；倒计时文本交给 View 的
/// TimelineView 每秒刷新；休息期间定期轮换壁纸。
@MainActor
final class BreakViewModel: ObservableObject {

    @Published private(set) var session: BreakSession?

    private let reminder: ReminderService
    private let wallpapers: WallpaperService
    private var rotationTimer: Timer?
    private var cancellables: Set<AnyCancellable> = []

    /// 休息期间背景缓慢换图的间隔
    private static let wallpaperRotationInterval: TimeInterval = 25

    init(reminder: ReminderService, wallpapers: WallpaperService) {
        self.reminder = reminder
        self.wallpapers = wallpapers

        reminder.$phase
            .receive(on: RunLoop.main)
            .sink { [weak self] phase in
                if case .breaking(let session) = phase {
                    self?.session = session
                }
            }
            .store(in: &cancellables)

        startRotation()
    }

    deinit {
        rotationTimer?.invalidate()
    }

    private func startRotation() {
        let timer = Timer(timeInterval: Self.wallpaperRotationInterval, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in self?.wallpapers.advance() }
        }
        RunLoop.main.add(timer, forMode: .common)
        rotationTimer = timer
    }

    /// 剩余时间 mm:ss（供 TimelineView 每秒刷新）
    func remainingText(now: Date) -> String {
        guard let session else { return "00:00" }
        return BreakSession.format(session.remaining(at: now))
    }

    var isAlmostDone: Bool {
        session.map { $0.remaining() <= 3 } ?? false
    }

    // MARK: - 用户意图

    func skip() {
        reminder.skipBreak()
    }
}
