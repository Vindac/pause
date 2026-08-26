import Combine
import Foundation

/// 跨窗口共享状态的轻量门面：组合各 Service / ViewModel，
/// 由 SwiftUI 注入 environment。不引入第三方依赖注入框架。
@MainActor
final class AppState: ObservableObject {

    let settings: SettingsStore
    let localization: LocalizationStore
    let wallpapers: WallpaperService
    let reminder: ReminderService
    let system: SystemActivityService
    let launchAtLogin: LaunchAtLoginService

    let reminderViewModel: ReminderViewModel
    let breakViewModel: BreakViewModel
    let menuBarViewModel: MenuBarViewModel
    let settingsViewModel: SettingsViewModel

    init(settings: SettingsStore,
         localization: LocalizationStore,
         wallpapers: WallpaperService,
         reminder: ReminderService,
         system: SystemActivityService,
         launchAtLogin: LaunchAtLoginService,
         reminderViewModel: ReminderViewModel,
         breakViewModel: BreakViewModel,
         menuBarViewModel: MenuBarViewModel,
         settingsViewModel: SettingsViewModel) {
        self.settings = settings
        self.localization = localization
        self.wallpapers = wallpapers
        self.reminder = reminder
        self.system = system
        self.launchAtLogin = launchAtLogin
        self.reminderViewModel = reminderViewModel
        self.breakViewModel = breakViewModel
        self.menuBarViewModel = menuBarViewModel
        self.settingsViewModel = settingsViewModel
    }
}
