import SwiftUI

/// 壁纸背景组件：aspectFill 铺满 + 极慢 Ken Burns 缩放 + 图片切换交叉淡化。
/// 设计要求：淡出 ~0.5s / 淡入 ~0.8s；scale 1.00 → 1.05。
struct WallpaperBackdropView: View {
    let image: NSImage?

    @State private var kenBurnsScale: CGFloat = 1.0

    private let crossfade = Animation.easeInOut(duration: 0.8)

    var body: some View {
        ZStack {
            Color.black
            if let image {
                layer(for: image)
                    .transition(.opacity)
            }
        }
        .animation(crossfade, value: image)
        .clipped()
        .onAppear {
            withAnimation(.easeInOut(duration: 45).repeatForever(autoreverses: true)) {
                kenBurnsScale = 1.05
            }
        }
    }

    private func layer(for image: NSImage) -> some View {
        Image(nsImage: image)
            .resizable()
            .aspectRatio(contentMode: .fill)
            .scaleEffect(kenBurnsScale)
            .ignoresSafeArea()
    }
}

/// 底部渐变遮罩，保证白色文字在任意壁纸上可读。
struct BottomReadabilityGradient: View {
    var body: some View {
        LinearGradient(
            colors: [.clear, .black.opacity(0.30), .black.opacity(0.72)],
            startPoint: .top, endPoint: .bottom
        )
        .ignoresSafeArea()
    }
}

/// 半透明材质感按钮（提醒 / 休息窗口用）。
struct GlassButtonStyle: ButtonStyle {
    var prominent = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 16, weight: prominent ? .semibold : .regular))
            .foregroundStyle(.white)
            .padding(.horizontal, 26)
            .padding(.vertical, 12)
            .background(
                (prominent ? Color.white.opacity(0.32) : Color.white.opacity(0.14))
                    .background(.ultraThinMaterial.opacity(0.35))
            )
            .clipShape(Capsule())
            .overlay(
                Capsule().strokeBorder(.white.opacity(0.35), lineWidth: 1)
            )
            .opacity(configuration.isPressed ? 0.7 : 1)
            .scaleEffect(configuration.isPressed ? 0.97 : 1)
            .animation(.easeOut(duration: 0.12), value: configuration.isPressed)
    }
}
