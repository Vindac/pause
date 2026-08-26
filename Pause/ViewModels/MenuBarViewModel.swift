import AppKit
import Combine
import Foundation

/// 菜单栏 ViewModel：把 ReminderService 状态转换为菜单栏标签与状态文案。
/// 菜单栏直接显示"距下次休息的剩余时间"（如 43m），休息中显示休息剩余。
@MainActor
final class MenuBarViewModel: ObservableObject {

    /// 菜单栏标签：43m（工作计时）/ 4m（休息中）/ !（待弹出）/ ⏸（暂停）
    @Published private(set) var barLabel: String = "--m"
    /// 菜单第一行状态文案
    @Published private(set) var statusTitle: String = ""
    @Published private(set) var isPaused: Bool = false

    private let reminder: ReminderService
    private let localization: LocalizationStore
    /// 打开设置窗口（由依赖容器注入：AppKit SettingsWindowController.open()）
    var openSettings: (() -> Void)?
    private var cancellables: Set<AnyCancellable> = []

    init(reminder: ReminderService, localization: LocalizationStore) {
        self.reminder = reminder
        self.localization = localization

        // 语言切换 / 空闲状态变化时状态文案也需要重算
        Publishers.CombineLatest(
            Publishers.CombineLatest3(
                reminder.$phase,
                reminder.$menuBarMinutes,
                reminder.$isWaitingForPresentation
            ),
            Publishers.CombineLatest(
                reminder.$isUserIdle,
                localization.$language
            )
        )
        .receive(on: RunLoop.main)
        .sink { [weak self] core, extra in
            let (phase, minutes, waiting) = core
            let (idle, _) = extra
            self?.render(phase: phase, minutes: minutes, waiting: waiting, idle: idle)
        }
        .store(in: &cancellables)
    }

    private func render(phase: ReminderPhase, minutes: Int?, waiting: Bool, idle: Bool) {
        isPaused = false
        switch phase {
        case .working:
            if waiting {
                barLabel = "!"
                statusTitle = localization.t(.statusWaiting)
            } else if let m = minutes {
                barLabel = "\(max(m, 0))m"
                statusTitle = idle
                    ? localization.t(.statusIdle)
                    : localization.t(.statusNextBreak(m))
            } else {
                barLabel = "--m"
                statusTitle = idle ? localization.t(.statusIdle) : ""
            }
        case .snoozing:
            barLabel = minutes.map { "\(max($0, 0))m" } ?? "--m"
            statusTitle = idle
                ? localization.t(.statusIdle)
                : minutes.map { localization.t(.statusSnoozed($0)) }
                    ?? localization.t(.statusReminding)
        case .reminding:
            barLabel = "!"
            statusTitle = localization.t(.statusReminding)
        case .breaking:
            barLabel = minutes.map { "\(max($0, 0))m" } ?? "--m"
            statusTitle = minutes.map { localization.t(.statusBreaking($0)) }
                ?? localization.t(.statusBreakingShort)
        case .paused:
            barLabel = "⏸"
            statusTitle = localization.t(.statusPaused)
            isPaused = true
        }
    }

    // MARK: - 用户意图

    func breakNow() {
        reminder.startBreak()
    }

    func togglePause() {
        if isPaused {
            reminder.resume()
        } else {
            reminder.pause()
        }
    }
}
