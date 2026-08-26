import SwiftUI

/// 提醒窗口的内容容器：按状态机阶段在「提醒页 / 休息页」之间切换。
/// 两页常驻（用透明度切换），保证窗口淡出时内容不闪空。
struct ReminderContainerView: View {
    @EnvironmentObject private var reminder: ReminderService

    var body: some View {
        ZStack {
            ReminderView()
                .opacity(reminder.phase.isRemindingPage ? 1 : 0)
                .allowsHitTesting(reminder.phase.isRemindingPage)

            BreakView()
                .opacity(reminder.phase.isBreakPage ? 1 : 0)
                .allowsHitTesting(reminder.phase.isBreakPage)
        }
        .animation(.easeInOut(duration: 0.4), value: reminder.phase)
    }
}

extension ReminderPhase {
    var isRemindingPage: Bool {
        if case .reminding = self { return true }
        return false
    }
    var isBreakPage: Bool {
        if case .breaking = self { return true }
        return false
    }
}
