import AppKit
import Combine
import Foundation

/// 系统状态服务：锁屏、屏保、屏幕睡眠、系统睡眠、前台全屏检测。
/// 用于：到点时若环境不适合打扰，则延后提醒。
final class SystemActivityService {

    static let shared = SystemActivityService()

    private var _isScreenLocked = false
    private var _isScreensaverActive = false
    private var _isScreenAsleep = false
    private let lock = NSLock()

    var isScreenLocked: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _isScreenLocked }
    }
    var isScreensaverActive: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _isScreensaverActive }
    }
    var isScreenAsleep: Bool {
        get { lock.lock(); defer { lock.unlock() }; return _isScreenAsleep }
    }

    private func setFlag(_ keyPath: ReferenceWritableKeyPath<SystemActivityService, Bool>, _ value: Bool) {
        lock.lock()
        defer { lock.unlock() }
        self[keyPath: keyPath] = value
    }

    private var observers: [NSObjectProtocol] = []

    private init() {
        installObservers()
    }

    deinit {
        let centers: [NotificationCenter] = [.default, DistributedNotificationCenter(), NSWorkspace.shared.notificationCenter]
        let obs = observers
        for center in centers {
            obs.forEach { center.removeObserver($0) }
        }
    }

    private func installObservers() {
        let dist = DistributedNotificationCenter()
        let ws = NSWorkspace.shared.notificationCenter

        observers.append(dist.addObserver(
            forName: NSNotification.Name("com.apple.screenIsLocked"), object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreenLocked, true) })

        observers.append(dist.addObserver(
            forName: NSNotification.Name("com.apple.screenIsUnlocked"), object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreenLocked, false) })

        observers.append(dist.addObserver(
            forName: NSNotification.Name("com.apple.screensaver.didstart"), object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreensaverActive, true) })

        observers.append(dist.addObserver(
            forName: NSNotification.Name("com.apple.screensaver.didstop"), object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreensaverActive, false) })

        observers.append(ws.addObserver(
            forName: NSWorkspace.screensDidSleepNotification, object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreenAsleep, true) })

        observers.append(ws.addObserver(
            forName: NSWorkspace.screensDidWakeNotification, object: nil, queue: nil
        ) { [weak self] _ in self?.setFlag(\._isScreenAsleep, false) })
    }

    /// 是否处于"弹出来也看不见"的状态（锁屏 / 屏保 / 屏幕睡眠）。
    /// 满足时提醒暂缓，环境恢复后 1 秒内自动弹出。
    /// 注意：不做前台全屏检测——几何判断极易把最大化大窗口误判为全屏，
    /// 导致用户活跃使用时提醒一直不弹；到点应直接弹出。
    var isPresentationBlocked: Bool {
        isScreenLocked || isScreensaverActive || isScreenAsleep
    }

    /// 距上次任意键鼠输入的秒数（活跃使用检测）。
    /// 只读取事件时间戳，不需要辅助功能权限；失败时返回 0（按活跃处理）。
    var userIdleSeconds: TimeInterval {
        guard let anyInput = CGEventType(rawValue: ~0) else { return 0 }   // kCGAnyInputEventType
        let seconds = CGEventSource.secondsSinceLastEventType(.combinedSessionState,
                                                              eventType: anyInput)
        return seconds.isFinite && seconds >= 0 ? seconds : 0
    }
}
