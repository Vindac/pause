import Combine
import Foundation
import AppKit

/// 系统状态的可测试边界（锁屏 / 屏保 / 屏幕睡眠 / 全屏演示 / 用户空闲）。
protocol SystemActivityProviding {
    var isPresentationBlocked: Bool { get }
    /// 距上次键鼠输入的秒数（用于"按真实使用时间计时"）
    var userIdleSeconds: TimeInterval { get }
}

extension SystemActivityService: SystemActivityProviding {}

/// 提醒核心状态机（Service）。
/// 唯一的计时真相源：所有窗口与菜单栏共享同一个 phase，避免多处 Timer 各自运行。
///
/// 睡眠/唤醒安全性：deadline 为绝对时间，系统睡眠期间 wall clock 照常推进，
/// 唤醒后的第一次 tick 只会将「已过期的一个 deadline」转换为一次提醒，天然不会风暴。
@MainActor
final class ReminderService: ObservableObject {

    @Published private(set) var phase: ReminderPhase
    /// 菜单栏分钟级倒计时（只在分钟数值变化时发布，避免高频刷新）
    @Published private(set) var menuBarMinutes: Int?
    /// 到点但因锁屏/屏保/全屏被暂缓（用于菜单栏提示）
    @Published private(set) var isWaitingForPresentation = false
    /// 开启「按真实使用时间计时」且当前处于离开状态（用于菜单栏提示）
    @Published private(set) var isUserIdle = false

    private let store: SettingsStore
    private let system: SystemActivityProviding
    private let clock: () -> Date
    private var timer: Timer?
    /// 上次 tick 的时刻（用于计算两次 tick 之间的墙钟流逝）
    private var lastTickAt: Date
    private var cancellables: Set<AnyCancellable> = []
    private var workspaceObservers: [NSObjectProtocol] = []

    init(store: SettingsStore,
         system: SystemActivityProviding = SystemActivityService.shared,
         clock: @escaping () -> Date = Date.init) {
        self.store = store
        self.system = system
        self.clock = clock
        let now = clock()
        self.lastTickAt = now
        self.phase = .working(deadline: Self.nextDeadline(
            now: now, intervalMinutes: store.reminderIntervalMinutes))

        // 修改提醒间隔后，从修改时重新开始当前工作周期
        store.$reminderIntervalMinutes
            .dropFirst()
            .removeDuplicates()
            .debounce(for: .milliseconds(300), scheduler: RunLoop.main)
            .sink { [weak self] _ in self?.restartWorkCycle() }
            .store(in: &cancellables)
    }

    deinit {
        timer?.invalidate()
        workspaceObservers.forEach {
            NSWorkspace.shared.notificationCenter.removeObserver($0)
        }
    }

    // MARK: - 纯逻辑（可测试）

    /// 下一轮工作周期 deadline
    static func nextDeadline(now: Date, intervalMinutes: Int) -> Date {
        now.addingTimeInterval(TimeInterval(intervalMinutes * 60))
    }

    /// 按真实使用时间计时：本 tick 内应顺延的秒数。
    /// 无输入达到阈值（离开/睡眠）→ 本段墙钟时间不计入使用时长；
    /// 顺延量取「空闲时长」与「距上次 tick 的墙钟流逝」的较小值。
    nonisolated static func idlePostponement(
        idleSeconds: TimeInterval, elapsed: TimeInterval,
        thresholdMinutes: Int, enabled: Bool) -> TimeInterval {
        guard enabled, elapsed > 0, idleSeconds >= TimeInterval(thresholdMinutes * 60) else { return 0 }
        return min(idleSeconds, elapsed)
    }

    // MARK: - 启动

    func start() {
        startTimer()
        observeWake()
    }

    private func startTimer() {
        timer?.invalidate()
        let t = Timer(timeInterval: 1.0, repeats: true) { [weak self] _ in
            Task { @MainActor [weak self] in
                self?.handleTick()
            }
        }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }

    /// 唤醒后立即 tick 一次，让过期的周期尽快（且只一次）转换为提醒
    private func observeWake() {
        let center = NSWorkspace.shared.notificationCenter
        workspaceObservers.append(center.addObserver(
            forName: NSWorkspace.didWakeNotification, object: nil, queue: .main
        ) { [weak self] _ in
            Task { @MainActor [weak self] in self?.handleTick() }
        })
    }

    // MARK: - 状态机核心

    /// 每秒调用一次：只做一次时间比较与分钟级发布，空闲开销可忽略。
    func handleTick(now date: Date? = nil) {
        let now = date ?? clock()
        let elapsed = now.timeIntervalSince(lastTickAt)
        lastTickAt = now
        let idle = system.userIdleSeconds
        let postpone = Self.idlePostponement(
            idleSeconds: idle, elapsed: elapsed,
            thresholdMinutes: store.idleThresholdMinutes,
            enabled: store.activityBasedTiming)
        updateIdleFlag(idle: idle)

        switch phase {
        case .working(let deadline), .snoozing(let deadline, _):
            // 离开/睡眠期间顺延 deadline，只累计真实使用时长
            let deadline = postpone > 0 ? deadline.addingTimeInterval(postpone) : deadline
            if case .snoozing(_, let count) = phase, postpone > 0 {
                phase = .snoozing(deadline: deadline, snoozeCount: count)
            } else if case .working = phase, postpone > 0 {
                phase = .working(deadline: deadline)
            }

            if deadline <= now {
                if system.isPresentationBlocked {
                    // 锁屏 / 屏保 / 全屏演示：暂缓，环境可用后下一 tick 自动弹出
                    isWaitingForPresentation = true
                } else {
                    fireReminder()
                }
            } else {
                isWaitingForPresentation = false
                publishMinutes(Int(ceil(deadline.timeIntervalSince(now) / 60)))
            }

        case .breaking(let session):
            let remaining = session.remaining(at: now)
            if remaining <= 0 {
                completeBreak()
            } else {
                publishMinutes(Int(ceil(remaining / 60)))
            }

        case .reminding(let autoBreakAt):
            // 自动开始休息：倒计时走完（期间用户未操作）自动进入休息
            if let autoBreakAt, now >= autoBreakAt {
                startBreak()
            }

        case .paused:
            break
        }
    }

    /// 空闲标记：仅在工作计时阶段有意义，值不变不重复发布
    private func updateIdleFlag(idle: TimeInterval) {
        let pendingWork: Bool
        switch phase {
        case .working, .snoozing: pendingWork = true
        default: pendingWork = false
        }
        let flag = pendingWork && store.activityBasedTiming
            && idle >= TimeInterval(store.idleThresholdMinutes * 60)
        if isUserIdle != flag { isUserIdle = flag }
    }

    private func publishMinutes(_ value: Int) {
        if menuBarMinutes != value { menuBarMinutes = value }
    }

    private func fireReminder() {
        isWaitingForPresentation = false
        // 开启"自动开始休息"时，提醒页显示倒计时并在到期后自动进入休息
        let autoBreakAt: Date? = store.autoStartBreak
            ? clock().addingTimeInterval(TimeInterval(store.autoStartBreakDelaySeconds))
            : nil
        phase = .reminding(autoBreakAt: autoBreakAt)
        if store.soundEnabled {
            NSSound(named: "Tink")?.play()
        }
    }

    /// 演示模式：立即进入提醒状态（隐藏功能，PAUSE_DEMO 环境变量触发）
    func demoTrigger() {
        fireReminder()
    }
    // MARK: - 用户动作

    /// 开始休息（提醒窗口与菜单栏"立即休息"共用；工作计时中也可直接进入）
    func startBreak() {
        if case .breaking = phase { return }
        let session = BreakSession(startedAt: clock(),
                                   duration: TimeInterval(store.breakDurationMinutes * 60))
        phase = .breaking(session)
    }

    /// 延迟休息：推迟 snoozeMinutes，且不重新计算完整工作间隔。
    /// 不限制次数（按钮常驻），仅记录本轮已延迟次数用于状态展示。
    func snooze() {
        guard case .reminding = phase else { return }
        snoozeCountThisCycle += 1
        phase = .snoozing(deadline: clock().addingTimeInterval(TimeInterval(store.snoozeMinutes * 60)),
                          snoozeCount: snoozeCountThisCycle)
    }

    /// 本轮已使用的 Snooze 次数（进入新工作周期时归零）
    private(set) var snoozeCountThisCycle = 0

    /// 休息自然结束或提前结束：关闭窗口并开始完整的新工作周期
    func completeBreak() {
        restartWorkCycle()
    }

    func skipBreak() {
        completeBreak()
    }

    /// 暂停全部提醒
    func pause() {
        guard case .paused = phase else {
            phase = .paused
            return
        }
    }

    /// 恢复：重新开始一个完整工作周期（简单可靠，不做秒级续算）
    func resume() {
        guard case .paused = phase else { return }
        restartWorkCycle()
    }

    func restartWorkCycle() {
        isWaitingForPresentation = false
        snoozeCountThisCycle = 0
        menuBarMinutes = store.reminderIntervalMinutes
        phase = .working(deadline: Self.nextDeadline(now: clock(),
                                                     intervalMinutes: store.reminderIntervalMinutes))
    }
}
