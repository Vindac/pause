import AppKit
import Combine
import Foundation

/// 提醒页 ViewModel：把壁纸与提醒状态转换为可展示状态，转发用户意图。
@MainActor
final class ReminderViewModel: ObservableObject {

    @Published private(set) var wallpaper: NSImage?
    @Published private(set) var breakDurationText: String
    @Published private(set) var snoozeButtonText: String

    private let wallpapers: WallpaperService
    private let reminder: ReminderService
    private let store: SettingsStore
    private let localization: LocalizationStore
    private var cancellables: Set<AnyCancellable> = []

    init(wallpapers: WallpaperService,
         reminder: ReminderService,
         store: SettingsStore,
         localization: LocalizationStore) {
        self.wallpapers = wallpapers
        self.reminder = reminder
        self.store = store
        self.localization = localization
        self.breakDurationText = ""
        self.snoozeButtonText = ""

        wallpapers.$current
            .receive(on: RunLoop.main)
            .sink { [weak self] item in self?.wallpaper = item?.image }
            .store(in: &cancellables)

        // 休息时长 / 延迟分钟 / 界面语言 任一变化即重算按钮与时长文案
        Publishers.CombineLatest3(
            store.$breakDurationMinutes,
            store.$snoozeMinutes,
            localization.$language
        )
        .receive(on: RunLoop.main)
        .sink { [weak self] breakMinutes, snoozeMinutes, _ in
            guard let self else { return }
            self.breakDurationText = self.localization.t(
                .reminderBreakFor(String(format: "%02d:00", breakMinutes)))
            self.snoozeButtonText = self.localization.t(.reminderDelay(snoozeMinutes))
        }
        .store(in: &cancellables)
    }

    /// 「开始休息」按钮文案：自动倒计时进行中附带剩余秒数（如「开始休息（3 秒）」）
    func startBreakText(now: Date) -> String {
        guard case .reminding(let autoBreakAt) = reminder.phase, let autoBreakAt else {
            return localization.t(.reminderStart)
        }
        let remaining = autoBreakAt.timeIntervalSince(now)
        guard remaining > 0 else { return localization.t(.reminderStart) }
        return localization.t(.reminderStartIn(Int(ceil(remaining))))
    }

    // MARK: - 用户意图

    func startBreak() {
        reminder.startBreak()
    }

    func snooze() {
        reminder.snooze()
    }
}
