import AppKit
import Combine
import Foundation

/// 壁纸调度服务：
/// 启动即从缓存装填当前图 → 后台预取下一张 → 提醒到达时立即切换到已就绪的本地图片。
/// 任何网络失败都沿「预取图 → 较旧缓存 → 运行时生成」降级，提醒永不等待网络。
@MainActor
final class WallpaperService: ObservableObject {

    @Published private(set) var current: WallpaperItem?

    private var prefetched: NSImage?
    private var prefetchTask: Task<Void, Never>?

    private let onlineProvider: WallpaperProviding
    private let cache: WallpaperCache
    private let store: SettingsStore
    private var cancellables: Set<AnyCancellable> = []

    init(store: SettingsStore,
         cache: WallpaperCache,
         onlineProvider: WallpaperProviding) {
        self.store = store
        self.cache = cache
        self.onlineProvider = onlineProvider

        // 取图地址变化：取消旧预取，按新地址重新预取
        store.$wallpaperImageURLString
            .dropFirst()
            .removeDuplicates()
            .debounce(for: .milliseconds(500), scheduler: RunLoop.main)
            .sink { [weak self] _ in
                self?.prefetchTask?.cancel()
                self?.prefetchNext()
            }
            .store(in: &cancellables)
    }

    // MARK: - 生命周期

    func bootstrap() {
        loadInitial()
        prefetchNext()
    }

    private func loadInitial() {
        if let latest = cache.latest(), let image = NSImage(contentsOfFile: latest.path) {
            current = WallpaperItem(origin: .networkCache(latest), image: image)
        } else {
            current = syncFallback()
        }
    }

    // MARK: - 切换

    /// 提醒触发 / 休息轮换 / 用户手动切换时调用：
    /// 展示下一张已就绪图片，再后台预取新的。
    func advance() {
        if let next = prefetched ?? fallbackFromCache() {
            prefetched = nil
            current = WallpaperItem(origin: .networkCache(cache.latest() ?? cache.directory), image: next)
        } else {
            current = syncFallback()
        }
        prefetchNext()
    }

    private func fallbackFromCache() -> NSImage? {
        if let url = cache.randomOlder(excludingFirst: cache.count > 1) {
            return NSImage(contentsOfFile: url.path)
        }
        return nil
    }

    /// 运行时渲染兜底图（CoreGraphics 生成，不依赖网络与缓存）
    private func syncFallback() -> WallpaperItem? {
        guard let image = BuiltInWallpaperProvider.render(theme: store.wallpaperTheme) else { return nil }
        return WallpaperItem(origin: .builtin(store.wallpaperTheme), image: image)
    }

    // MARK: - 预取

    private func prefetchNext() {
        prefetchTask?.cancel()
        let url = store.wallpaperImageURL
        prefetchTask = Task { [weak self] in
            guard let self else { return }
            let image = await self.onlineProvider.fetchNext(url: url)
            if !Task.isCancelled, let image {
                self.prefetched = image
            }
        }
    }
}
