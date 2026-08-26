import SwiftUI

/// 到点提醒页：全幅壁纸 + 渐变遮罩 + 标题文案 + 休息时长 + 两按钮。
struct ReminderView: View {
    @EnvironmentObject private var vm: ReminderViewModel
    @EnvironmentObject private var l10n: LocalizationStore

    var body: some View {
        ZStack {
            WallpaperBackdropView(image: vm.wallpaper)
            BottomReadabilityGradient()

            VStack(spacing: 20) {
                Spacer()

                Text(l10n.t(.reminderTitle))
                    .font(.system(size: 42, weight: .bold, design: .rounded))
                    .foregroundStyle(.white)
                    .shadow(color: .black.opacity(0.45), radius: 10, y: 2)

                Text(l10n.t(.reminderSubtitle))
                    .font(.system(size: 20, weight: .regular))
                    .foregroundStyle(.white.opacity(0.92))
                    .shadow(color: .black.opacity(0.4), radius: 6, y: 1)

                Spacer()

                Text(vm.breakDurationText)
                    .font(.system(size: 17, weight: .medium).monospacedDigit())
                    .foregroundStyle(.white.opacity(0.95))
                    .shadow(color: .black.opacity(0.4), radius: 5)

                HStack(spacing: 16) {
                    Button(vm.snoozeButtonText) { vm.snooze() }
                        .buttonStyle(GlassButtonStyle())

                    // 开启"自动开始休息"时按钮文案附带逐秒倒计时（手动模式纯文案）
                    TimelineView(.periodic(from: .now, by: 1)) { timeline in
                        Button(vm.startBreakText(now: timeline.date)) { vm.startBreak() }
                            .buttonStyle(GlassButtonStyle(prominent: true))
                            .monospacedDigit()
                    }
                }
            }
            .padding(.bottom, 52)
            .padding(.top, 40)
        }
    }
}
