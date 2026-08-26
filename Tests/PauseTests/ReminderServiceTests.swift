import XCTest
@testable import Pause

/// 可控时钟
final class FakeClock {
    var now: Date
    init(start: Date) { self.now = start }
    func advance(_ seconds: TimeInterval) { now = now.addingTimeInterval(seconds) }
}

/// 可控系统状态：默认"环境可打扰、用户活跃"
final class FakeSystemActivity: SystemActivityProviding {
    var blocked = false
    var isPresentationBlocked: Bool { blocked }
    /// 距上次键鼠输入的秒数（0 = 活跃）
    var idleSeconds: TimeInterval = 0
    var userIdleSeconds: TimeInterval { idleSeconds }
}

/// 提醒状态机纯逻辑测试：计时推进、Snooze 限制、暂停恢复、唤醒防风暴、锁屏延后。
@MainActor
final class ReminderServiceTests: XCTestCase {

    private var clock: FakeClock!
    private var activity: FakeSystemActivity!

    override func setUp() {
        super.setUp()
        clock = FakeClock(start: Date(timeIntervalSince1970: 1_000_000))
        activity = FakeSystemActivity()
    }

    private func makeDefaults(interval: Int = 45, break_: Int = 5, snooze: Int = 5,
                              activityTiming: Bool = true, idleThreshold: Int = 2,
                              autoStart: Bool = false, autoStartDelay: Int = 30) -> UserDefaults {
        let name = "test.\(UUID().uuidString)"
        let d = UserDefaults(suiteName: name)!
        d.removePersistentDomain(forName: name)
        d.set(interval, forKey: SettingsStore.Key.reminderIntervalMinutes)
        d.set(break_, forKey: SettingsStore.Key.breakDurationMinutes)
        d.set(snooze, forKey: SettingsStore.Key.snoozeMinutes)
        d.set(activityTiming, forKey: SettingsStore.Key.activityBasedTiming)
        d.set(idleThreshold, forKey: SettingsStore.Key.idleThresholdMinutes)
        d.set(autoStart, forKey: SettingsStore.Key.autoStartBreak)
        d.set(autoStartDelay, forKey: SettingsStore.Key.autoStartBreakDelaySeconds)
        d.set(false, forKey: SettingsStore.Key.soundEnabled)
        return d
    }

    private func makeService(interval: Int = 45, break_: Int = 5, snooze: Int = 5,
                             activityTiming: Bool = true, idleThreshold: Int = 2,
                             autoStart: Bool = false, autoStartDelay: Int = 30) -> ReminderService {
        ReminderService(
            store: SettingsStore(defaults: makeDefaults(
                interval: interval, break_: break_, snooze: snooze,
                activityTiming: activityTiming, idleThreshold: idleThreshold,
                autoStart: autoStart, autoStartDelay: autoStartDelay)),
            system: activity,
            clock: { [clock] in clock.now }
        )
    }

    // MARK: - 工作周期

    func testInitialPhaseIsWorkingWithIntervalDeadline() {
        let start = clock.now
        let service = makeService(interval: 45)
        guard case .working(let deadline) = service.phase else {
            return XCTFail("expected working, got \(service.phase)")
        }
        XCTAssertEqual(deadline.timeIntervalSince(start), 45 * 60, accuracy: 1)
    }

    func testTickBeforeDeadlineDoesNotFire() {
        let service = makeService()
        clock.advance(60)
        service.handleTick(now: clock.now)
        guard case .working = service.phase else {
            return XCTFail("should still be working")
        }
        XCTAssertEqual(service.isWaitingForPresentation, false)
    }

    func testTickAtDeadlineFiresReminder() {
        let service = makeService()
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        guard case .reminding = service.phase else {
            return XCTFail("should be reminding, got \(service.phase)")
        }
    }

    /// 锁屏 / 全屏演示时到期不弹，解锁后下一 tick 弹出
    func testBlockedPresentationDefersReminder() {
        let service = makeService()
        clock.advance(45 * 60 + 1)
        activity.blocked = true
        service.handleTick(now: clock.now)
        guard case .working = service.phase else { return XCTFail("should defer, got \(service.phase)") }
        XCTAssertTrue(service.isWaitingForPresentation)

        clock.advance(2)
        activity.blocked = false
        service.handleTick(now: clock.now)
        guard case .reminding = service.phase else {
            return XCTFail("should fire after unblocked, got \(service.phase)")
        }
    }

    // MARK: - Snooze

    func testSnoozePostponesWithoutFullInterval() {
        let start = clock.now
        let service = makeService(snooze: 5)
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        service.snooze()
        guard case .snoozing(let deadline, let count) = service.phase else {
            return XCTFail("should be snoozing, got \(service.phase)")
        }
        XCTAssertEqual(count, 1)
        // 推迟 5 分钟，而不是重新计算 45 分钟
        XCTAssertEqual(deadline.timeIntervalSince(start), 45 * 60 + 5 * 60, accuracy: 2)
    }

    /// 延迟不再限制次数：第 4 次及以后仍然允许
    func testSnoozeIsNotLimited() {
        let service = makeService(snooze: 5)
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)

        for expectedCount in 1...5 {
            service.snooze()
            guard case .snoozing(_, let count) = service.phase else {
                return XCTFail("snooze #\(expectedCount) should be allowed")
            }
            XCTAssertEqual(count, expectedCount)
            clock.advance(5 * 60 + 1)          // snooze 到期再次提醒
            service.handleTick(now: clock.now)
            guard case .reminding = service.phase else {
                return XCTFail("should remind again after snooze")
            }
        }
    }

    func testSnoozeCountResetsAfterNewWorkCycle() {
        let service = makeService()
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        service.snooze()
        clock.advance(5 * 60 + 1)
        service.handleTick(now: clock.now)
        service.startBreak()
        clock.advance(5 * 60 + 1)              // 休息结束
        service.handleTick(now: clock.now)
        XCTAssertEqual(service.snoozeCountThisCycle, 0)
        guard case .working = service.phase else { return XCTFail("should restart working") }
    }

    // MARK: - 休息

    func testBreakCompletesAndRestartsWorkCycleFromBreakEnd() {
        let service = makeService()
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        service.startBreak()
        guard case .breaking(let session) = service.phase else { return XCTFail() }
        XCTAssertEqual(session.duration, 5 * 60, accuracy: 1)

        clock.advance(5 * 60 + 1)
        service.handleTick(now: clock.now)
        guard case .working(let deadline) = service.phase else {
            return XCTFail("expected new working cycle, got \(service.phase)")
        }
        // 新周期从休息结束时重新开始 45 分钟
        XCTAssertEqual(deadline.timeIntervalSince(session.endsAt), 45 * 60, accuracy: 2)
    }

    func testSkipBreakRestartsImmediately() {
        let service = makeService()
        service.startBreak()
        service.skipBreak()
        guard case .working = service.phase else { return XCTFail() }
    }

    // MARK: - 暂停

    func testPauseAndResume() {
        let service = makeService()
        service.pause()
        guard case .paused = service.phase else { return XCTFail() }
        clock.advance(10 * 3600)               // 暂停期间不触发
        service.handleTick(now: clock.now)
        guard case .paused = service.phase else { return XCTFail() }
        service.resume()
        guard case .working(let deadline) = service.phase else { return XCTFail() }
        XCTAssertEqual(deadline.timeIntervalSince(clock.now), 45 * 60, accuracy: 1)
    }

    // MARK: - 睡眠唤醒

    /// 睡眠唤醒防风暴：deadline 已经过去很久，一次 tick 只触发一次提醒
    func testWakeAfterLongSleepFiresOnlyOneReminder() {
        let service = makeService()
        clock.advance(8 * 3600)                // 模拟睡眠 8 小时后唤醒
        service.handleTick(now: clock.now)
        guard case .reminding = service.phase else {
            return XCTFail("should fire once, got \(service.phase)")
        }
        service.startBreak()
        clock.advance(5 * 60 + 1)
        service.handleTick(now: clock.now)
        guard case .working = service.phase else { return XCTFail("no storm: back to working") }
    }

    // MARK: - 间隔修改

    func testRestartUsesCurrentInterval() {
        let service = makeService(interval: 30)
        let before = clock.now
        service.restartWorkCycle()
        guard case .working(let deadline) = service.phase else { return XCTFail() }
        XCTAssertEqual(deadline.timeIntervalSince(before), 30 * 60, accuracy: 1)
    }

    // MARK: - 纯函数

    func testNextDeadlinePureFunction() {
        let now = Date(timeIntervalSince1970: 500)
        let d = ReminderService.nextDeadline(now: now, intervalMinutes: 10)
        XCTAssertEqual(d.timeIntervalSince(now), 600, accuracy: 0.01)
    }

    // MARK: - 自动开始休息

    /// 开启自动开始休息：提醒带倒计时，期间不动作，倒计时结束自动进入休息
    func testAutoStartBreakBeginsBreakAfterCountdown() {
        let service = makeService(autoStart: true, autoStartDelay: 30)
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        guard case .reminding(let autoBreakAt) = service.phase else { return XCTFail() }
        XCTAssertEqual(autoBreakAt?.timeIntervalSince(clock.now) ?? 0, 30, accuracy: 1)

        clock.advance(29)                       // 倒计时未结束：仍为提醒
        service.handleTick(now: clock.now)
        guard case .reminding = service.phase else { return XCTFail("倒计时未到不应自动开始") }

        clock.advance(2)                        // 越过倒计时：自动开始休息
        service.handleTick(now: clock.now)
        guard case .breaking = service.phase else { return XCTFail("倒计时结束应自动开始休息") }
    }

    /// 手动模式：提醒永不自动开始休息
    func testManualModeNeverAutoStarts() {
        let service = makeService(autoStart: false)
        clock.advance(45 * 60 + 1)
        service.handleTick(now: clock.now)
        guard case .reminding(let autoBreakAt) = service.phase else { return XCTFail() }
        XCTAssertNil(autoBreakAt)

        clock.advance(3600)                     // 挂 1 小时也不自动开始
        service.handleTick(now: clock.now)
        guard case .reminding = service.phase else { return XCTFail("手动模式不应自动开始休息") }
    }

    // MARK: - 按真实使用时间计时

    /// 空闲达到阈值：本段墙钟时间不计入，deadline 相应顺延
    func testIdlePostponesDeadline() {
        let start = clock.now
        let service = makeService()                  // 阈值 2 分钟，默认开启
        clock.advance(10 * 60)                       // 工作 10 分钟
        service.handleTick(now: clock.now)           // 活跃 tick，deadline = start+45min
        XCTAssertEqual(service.menuBarMinutes, 35)

        activity.idleSeconds = 8 * 60                // 离开，已空闲 8 分钟 ≥ 2 分钟阈值
        clock.advance(6 * 60)                        // 墙钟又过 6 分钟
        service.handleTick(now: clock.now)           // 顺延 min(480, 360)=360s
        guard case .working(let deadline) = service.phase else { return XCTFail() }
        // 使用时长仍为 10 分钟：deadline = start + 45min + 6min 顺延
        XCTAssertEqual(deadline.timeIntervalSince(start), 45 * 60 + 6 * 60, accuracy: 2)
        XCTAssertEqual(service.menuBarMinutes, 35)
        XCTAssertTrue(service.isUserIdle)
    }

    /// 空闲未达阈值：正常计时
    func testIdleBelowThresholdDoesNotPostpone() {
        let start = clock.now
        let service = makeService()                  // 阈值 2 分钟
        clock.advance(10 * 60)
        activity.idleSeconds = 90                    // < 120s 阈值，仍视为活跃
        service.handleTick(now: clock.now)
        guard case .working(let deadline) = service.phase else { return XCTFail() }
        XCTAssertEqual(deadline.timeIntervalSince(start), 45 * 60, accuracy: 1)
        XCTAssertFalse(service.isUserIdle)
    }

    /// 关闭按使用时间计时：空闲也不顺延（回到固定时间模式）
    func testActivityTimingDisabledDoesNotPostpone() {
        let start = clock.now
        let service = makeService(activityTiming: false)
        clock.advance(10 * 60)
        activity.idleSeconds = 30 * 60
        service.handleTick(now: clock.now)
        guard case .working(let deadline) = service.phase else { return XCTFail() }
        XCTAssertEqual(deadline.timeIntervalSince(start), 45 * 60, accuracy: 1)
        XCTAssertFalse(service.isUserIdle)
    }

    /// 睡眠唤醒（空闲很长、elapsed 很长）：deadline 顺延整个空闲期，唤醒后不立即提醒
    func testWakeAfterLongIdleDoesNotFireImmediately() {
        let service = makeService()
        clock.advance(5 * 60)
        service.handleTick(now: clock.now)           // 活跃中
        activity.idleSeconds = 8 * 3600              // 睡眠 8 小时
        clock.advance(8 * 3600)
        service.handleTick(now: clock.now)           // 唤醒后第一次 tick
        guard case .working = service.phase else {
            return XCTFail("deadline should be postponed by the sleep duration")
        }
        // 顺延 8 小时后仍剩约 40 分钟使用时长
        XCTAssertEqual(service.menuBarMinutes, 40)
    }

    /// 空闲顺延纯函数
    func testIdlePostponementPureFunction() {
        // 关闭功能
        XCTAssertEqual(ReminderService.idlePostponement(
            idleSeconds: 600, elapsed: 60, thresholdMinutes: 2, enabled: false), 0)
        // 空闲未达阈值
        XCTAssertEqual(ReminderService.idlePostponement(
            idleSeconds: 100, elapsed: 60, thresholdMinutes: 2, enabled: true), 0)
        // 正常离开：取较小的墙钟流逝
        XCTAssertEqual(ReminderService.idlePostponement(
            idleSeconds: 600, elapsed: 60, thresholdMinutes: 2, enabled: true), 60)
        // 唤醒场景：idle 大于 elapsed
        XCTAssertEqual(ReminderService.idlePostponement(
            idleSeconds: 8 * 3600, elapsed: 300, thresholdMinutes: 2, enabled: true), 300)
    }

    /// 分钟级发布：值不变时不重复发布
    func testMenuBarMinutesPublishesMinuteValues() {
        let service = makeService()
        clock.advance(10)
        service.handleTick(now: clock.now)
        XCTAssertEqual(service.menuBarMinutes, 45)
        clock.advance(30)
        service.handleTick(now: clock.now)
        XCTAssertEqual(service.menuBarMinutes, 45)   // 仍在本分钟
        clock.advance(60)
        service.handleTick(now: clock.now)
        XCTAssertEqual(service.menuBarMinutes, 44)
    }
}
