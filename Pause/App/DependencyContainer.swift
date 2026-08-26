import Combine
import Foundation
import SwiftUI

/// 依赖组装：构建全部 Service / ViewModel / 窗口控制器并完成订阅连线。
/// 保持单向数据流：View → ViewModel → Service → Model/系统。
@MainActor
final class DependencyContainer {

    let settings: SettingsStore
    let localization: LocalizationStore
    let cache: WallpaperCache
    let wallpapers: WallpaperService
    let reminder: ReminderService
    let system: SystemActivityService
    let launchAtLogin: LaunchAtLoginService

    let reminderViewModel: ReminderViewModel
    let breakViewModel: BreakViewModel
    let menuBarViewModel: MenuBarViewModel
    let settingsViewModel: SettingsViewModel

    let windowController: ReminderWindowController
    let settingsWindowController: SettingsWindowController
    let appState: AppState

    private var cancellables: Set<AnyCancellable> = []

    init() {
        let settings = SettingsStore()
        self.settings = settings
        let localization = LocalizationStore()
        self.localization = localization
        self.system = SystemActivityService.shared
        self.launchAtLogin = LaunchAtLoginService()

        let cache = WallpaperCache()
        self.cache = cache
        let wallpapers = WallpaperService(
            store: settings,
            cache: cache,
            onlineProvider: OnlineWallpaperProvider(cache: cache)
        )
        self.wallpapers = wallpapers

        let reminder = ReminderService(store: settings, system: system)
        self.reminder = reminder

        self.reminderViewModel = ReminderViewModel(
            wallpapers: wallpapers, reminder: reminder,
            store: settings, localization: localization)
        self.breakViewModel = BreakViewModel(reminder: reminder, wallpapers: wallpapers)
        self.menuBarViewModel = MenuBarViewModel(reminder: reminder, localization: localization)
        self.settingsViewModel = SettingsViewModel(
            store: settings, launchAtLoginService: launchAtLogin,
            wallpapers: wallpapers, localization: localization)

        let windowContent = ReminderContainerView()
            .environmentObject(reminder)
            .environmentObject(wallpapers)
            .environmentObject(reminderViewModel)
            .environmentObject(breakViewModel)
            .environmentObject(localization)
        self.windowController = ReminderWindowController(contentView: windowContent)

        let settingsWindowController = SettingsWindowController(
            viewModel: settingsViewModel, localization: localization)
        self.settingsWindowController = settingsWindowController

        // 菜单栏「设置…」→ 打开 AppKit 设置窗口
        menuBarViewModel.openSettings = { [weak settingsWindowController] in
            settingsWindowController?.open()
        }

        self.appState = AppState(
            settings: settings,
            localization: localization,
            wallpapers: wallpapers,
            reminder: reminder,
            system: system,
            launchAtLogin: launchAtLogin,
            reminderViewModel: reminderViewModel,
            breakViewModel: breakViewModel,
            menuBarViewModel: menuBarViewModel,
            settingsViewModel: settingsViewModel
        )
    }

    /// 应用启动完成后的接线与启动（只调用一次）
    func start() {
        wallpapers.bootstrap()
        reminder.start()
        syncLaunchAtLogin()

        // 状态机 → 窗口 / 壁纸 编排
        reminder.$phase
            .receive(on: RunLoop.main)
            .sink { [weak self] phase in
                guard let self else { return }
                switch phase {
                case .reminding:
                    self.wallpapers.advance()
                    self.windowController.show(
                        on: ReminderWindowController.screenForPresentation(),
                        overlay: self.settings.overlayOtherWindows,
                        opacity: self.settings.reminderWindowOpacity)
                case .breaking:
                    // 提醒页 → 休息页复用同一窗口；"立即休息"则新弹出
                    self.windowController.ensureVisible(
                        on: ReminderWindowController.screenForPresentation(),
                        overlay: self.settings.overlayOtherWindows,
                        opacity: self.settings.reminderWindowOpacity)
                case .working, .snoozing, .paused:
                    self.windowController.hide()
                }
            }
            .store(in: &cancellables)

        // 透明度设置变化：已显示的提醒窗口立即平滑应用
        settings.$reminderWindowOpacity
            .dropFirst()
            .debounce(for: .milliseconds(50), scheduler: RunLoop.main)
            .sink { [weak self] opacity in
                self?.windowController.applyOpacity(opacity)
            }
            .store(in: &cancellables)
    }

    /// 让 SMAppService 状态与设置保持一致（裸二进制运行时注册会失败，仅记录）
    private func syncLaunchAtLogin() {
        let desired = settings.launchAtLogin
        let current = launchAtLogin.isEnabled
        if desired != current {
            launchAtLogin.setEnabled(desired)
        }
    }
}
