import Foundation
import ServiceManagement

/// 开机启动服务：基于 SMAppService（macOS 13+）。
/// 注意：只有在 .app bundle 内运行时注册才会成功；裸 swift run 可执行文件会失败但不影响其他功能。
final class LaunchAtLoginService {

    private var isRegistering = false

    var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    /// 注册 / 注销开机启动；返回是否成功
    @discardableResult
    func setEnabled(_ enabled: Bool) -> Bool {
        guard !isRegistering else { return false }
        isRegistering = true
        defer { isRegistering = false }

        do {
            if enabled {
                if SMAppService.mainApp.status != .enabled {
                    try SMAppService.mainApp.register()
                }
            } else {
                if SMAppService.mainApp.status == .enabled {
                    try SMAppService.mainApp.unregister()
                }
            }
            return true
        } catch {
            NSLog("[Pause] LaunchAtLogin %@ failed: %@",
                  enabled ? "register" : "unregister", error.localizedDescription)
            return false
        }
    }
}
