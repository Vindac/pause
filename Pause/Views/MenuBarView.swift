import SwiftUI

/// 菜单栏下拉菜单内容。
struct MenuBarView: View {
    @EnvironmentObject private var vm: MenuBarViewModel
    @EnvironmentObject private var l10n: LocalizationStore

    var body: some View {
        VStack {
            Text(vm.statusTitle)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(.secondary)

            Divider()

            Button(l10n.t(.menuBreakNow)) { vm.breakNow() }
            Button(vm.isPaused ? l10n.t(.menuResume) : l10n.t(.menuPause)) { vm.togglePause() }
                .keyboardShortcut("p")

            Divider()

            Button(l10n.t(.menuSettings)) {
                DebugLog.log("menu: 设置 tapped")
                vm.openSettings?()
            }
                .keyboardShortcut(",")

            Divider()

            Button(l10n.t(.menuQuit)) { NSApp.terminate(nil) }
                .keyboardShortcut("q")
        }
    }
}
