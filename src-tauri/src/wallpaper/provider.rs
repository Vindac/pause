//! OnlineWallpaperProvider.swift 的 Rust 移植：
//! 下载（禁缓存语义）、HTTP 200 + ≥30KB 校验、最多尝试 2 次，
//! 成功即落盘进缓存并返回本地路径。`WallpaperProviding.fetchNext`
//! 对应 async fetch_next —— 返回 None 表示失败，由调用方降级。

use super::cache::WallpaperCache;
use std::sync::Arc;
use std::time::Duration;

pub struct OnlineWallpaperProvider {
    client: reqwest::Client,
}

impl OnlineWallpaperProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("reqwest client");
        Self { client }
    }

    pub fn default_url() -> &'static str {
        crate::settings::DEFAULT_WALLPAPER_URL
    }

    /// 每次请求必须拿到"新图"，即使地址相同（reloadIgnoringLocalCacheData 语义
    /// 由 header no-cache 表达）。
    pub async fn fetch_next(
        &self,
        url: Option<&str>,
        cache: Arc<WallpaperCache>,
    ) -> Option<std::path::PathBuf> {
        let url = url
            .map(str::to_string)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Self::default_url().to_string());

        for _ in 0..2 {
            let req = self
                .client
                .get(&url)
                .header("Cache-Control", "no-cache")
                .header("Pragma", "no-cache");
            match req.send().await {
                Ok(resp) if resp.status().as_u16() == 200 => {
                    let Ok(bytes) = resp.bytes().await else {
                        continue;
                    };
                    // 校验：状态码 200 且体积 > 30_000 字节（挡错误页）
                    if bytes.len() <= 30_000 {
                        continue;
                    }
                    if let Some(path) = cache.add(&bytes) {
                        return Some(path);
                    }
                }
                _ => continue,
            }
        }
        None
    }
}

impl Default for OnlineWallpaperProvider {
    fn default() -> Self {
        Self::new()
    }
}
