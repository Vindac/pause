import SwiftUI

/// 休息倒计时页：柔和背景 + 简短引导文案 + mm:ss 倒计时 + 提前结束。
struct BreakView: View {
    @EnvironmentObject private var vm: BreakViewModel
    @EnvironmentObject private var wallpapers: WallpaperService
    @EnvironmentObject private var l10n: LocalizationStore

    var body: some View {
        ZStack {
            WallpaperBackdropView(image: wallpapers.current?.image)
            BottomReadabilityGradient()

            TimelineView(.periodic(from: .now, by: 1)) { timeline in
                VStack(spacing: 22) {
                    Spacer()

                    Text("🌲")
                        .font(.system(size: 56))
                        .shadow(color: .black.opacity(0.35), radius: 8, y: 2)

                    Text(l10n.t(.breakTitle))
                        .font(.system(size: 32, weight: .semibold, design: .rounded))
                        .foregroundStyle(.white)
                        .shadow(color: .black.opacity(0.4), radius: 8, y: 2)

                    Text(vm.remainingText(now: timeline.date))
                        .font(.system(size: 64, weight: .light).monospacedDigit())
                        .foregroundStyle(.white)
                        .shadow(color: .black.opacity(0.45), radius: 10, y: 2)

                    Text(vm.isAlmostDone ? l10n.t(.breakAlmostDone) : l10n.t(.breakHint))
                        .font(.system(size: 18))
                        .foregroundStyle(.white.opacity(0.9))
                        .shadow(color: .black.opacity(0.35), radius: 5)

                    Spacer()

                    Button(l10n.t(.breakSkip)) { vm.skip() }
                        .buttonStyle(GlassButtonStyle())
                }
                .padding(.bottom, 48)
            }
        }
    }
}
