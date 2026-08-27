//! WallpaperService.swift 的 Rust 移植：壁纸调度。
//!
//! - bootstrap()：启动即从缓存装填当前图并后台预取下一张；
//!   提醒到达时**只使用已就绪的本地图片**，从不等待网络；
//! - advance()：预取槽 → 缓存较旧随机 → 缓存最新 → 兜底渲染，
//!   全部同步内存/磁盘操作；完成后立即补位下一张的网络预取；
//! - 自定义 URL 变更后调用 refetch()：旧任务结果按代际号自动作废。

pub mod cache;
pub mod fallback;
pub mod provider;

use crate::settings::{SharedSettings, WallpaperTheme};
use cache::WallpaperCache;
use provider::OnlineWallpaperProvider;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

/// 预取完成后的落槽目标：代际匹配才写入（过期任务静默丢弃）。
type Slot = Arc<Mutex<Option<PathBuf>>>;
type SharedGeneration = Arc<AtomicU64>;

pub struct WallpaperService {
    store: SharedSettings,
    pub cache: Arc<WallpaperCache>,
    online: Arc<OnlineWallpaperProvider>,
    /// 当前展示的本地图片路径。
    current: RwLock<Option<PathBuf>>,
    /// 单槽预取：同时只有 1 张在途。
    prefetched: Slot,
    generation: SharedGeneration,
}

impl WallpaperService {
    pub fn new(store: SharedSettings, cache_dir: &std::path::Path) -> Self {
        Self {
            store,
            cache: Arc::new(WallpaperCache::new(cache_dir)),
            online: Arc::new(OnlineWallpaperProvider::new()),
            current: RwLock::new(None),
            prefetched: Arc::new(Mutex::new(None)),
            generation: Arc::new(AtomicU64::new(1)),
        }
    }

    fn effective_url(&self) -> String {
        self.store
            .lock()
            .unwrap()
            .wallpaper_image_url_string
            .clone()
    }

    /// 兜底渐变图渲染到临时目录（首启无缓存、无网络的最终保障）。
    fn render_fallback_to_temp(&self) -> PathBuf {
        let theme = { self.store.lock().unwrap().wallpaper_theme };
        let dest = std::env::temp_dir().join(format!(
            "pause-fallback-{}-{}.png",
            theme_file_name(theme),
            uuid::Uuid::new_v4()
        ));
        if let Err(err) =
            fallback::render(theme).save_with_format(&dest, image::ImageFormat::Png)
        {
            crate::debug_log::debug_log(&format!("fallback render failed: {err}"));
        }
        dest
    }

    /// 启动装填：缓存最新一张命中则用之；否则兜底渲染。随后立即预取下一张。
    pub fn bootstrap(&self) -> PathBuf {
        let loaded = self
            .cache
            .latest()
            .unwrap_or_else(|| self.render_fallback_to_temp());
        *self.current.write().unwrap() = Some(loaded.clone());
        self.spawn_prefetch();
        loaded
    }

    /// 切到下一张：优先预取槽 → 缓存较旧随机 → 缓存最新 → 兜底。
    /// 完全同步、永不等网络 —— 与原版"只使用已就绪的本地图片"对齐。
    pub fn advance(&self) -> PathBuf {
        // 使任何在途预取失效
        self.generation.fetch_add(1, Ordering::Relaxed);

        let next = { self.prefetched.lock().unwrap().take() };
        let next = next
            .filter(|p| p.exists())
            .or_else(|| self.cache.random_older())
            .or_else(|| self.cache.latest())
            .unwrap_or_else(|| self.render_fallback_to_temp());

        *self.current.write().unwrap() = Some(next.clone());
        crate::debug_log::debug_log(&format!("advanced to {}", next.display()));

        self.spawn_prefetch(); // 立即补位下一张
        next
    }

    /// 发起一次后台预取；完成后若期间没有新的 advance/refetch 才入槽。
    pub fn spawn_prefetch(&self) {
        let provider = Arc::clone(&self.online);
        let cache = Arc::clone(&self.cache);
        let slot: Slot = Arc::clone(&self.prefetched);
        let generation: SharedGeneration = Arc::clone(&self.generation);
        let gen_at_spawn = generation.load(Ordering::Relaxed);
        let url = self.effective_url();

        tauri::async_runtime::spawn(async move {
            let fetched = provider.fetch_next(Some(url.as_str()), cache).await;
            if let Some(path) = fetched {
                if generation.load(Ordering::Relaxed) == gen_at_spawn {
                    *slot.lock().unwrap() = Some(path);
                }
            }
        });
    }

    pub fn current_path(&self) -> Option<PathBuf> {
        self.current.read().unwrap().clone()
    }
}

fn theme_file_name(t: WallpaperTheme) -> &'static str {
    crate::settings::theme_name(t)
}
