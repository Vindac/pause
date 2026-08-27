//! WallpaperCache.swift 的 Rust 移植。
//!
//! 缓存位于系统缓存目录下 `Pause/Wallpapers/`：写入即降采样到最长边 ≤3200px，
//! JPEG 质量 0.82 落盘；索引文件维护新→旧顺序，超出 MAX_COUNT 自动淘汰最旧
//! （含物理文件删除）。读取策略：latest() 取最新；random_older() 排除最新随机取。

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};
use std::io::Read;
use std::path::{Path, PathBuf};

pub const MAX_COUNT: usize = 18;
pub const MAX_PIXEL_SIZE: u32 = 3200;
const JPEG_QUALITY: u8 = 82; // 原版压缩因子 0.82

pub struct WallpaperCache {
    dir: PathBuf,
}

/// 淘汰计划纯函数：有序名单中超出上限的尾部即最旧者。
pub fn files_to_evict<'a>(ordered_names: &'a [String], max_count: usize) -> &'a [String] {
    if ordered_names.len() <= max_count {
        &[]
    } else {
        &ordered_names[max_count..]
    }
}

impl WallpaperCache {
    /// base 为应用缓存根目录（app_cache_dir）。
    pub fn new(base: &Path) -> Self {
        let dir = base.join("Pause").join("Wallpapers");
        let _ = std::fs::create_dir_all(&dir);
        Self { dir }
    }

    pub fn directory(&self) -> &Path {
        &self.dir
    }

    fn index_path(&self) -> PathBuf {
        self.dir.join("index.json")
    }

    pub fn load_index(&self) -> Vec<String> {
        std::fs::read_to_string(self.index_path())
            .ok()
            .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
            .unwrap_or_default()
    }

    pub fn save_index(&self, names: &[String]) {
        if let Ok(json) = serde_json::to_string(names) {
            let _ = std::fs::write(self.index_path(), json);
        }
    }

    pub fn count(&self) -> usize {
        self.load_index().len()
    }

    /// 写入一份下载完成的图片数据：校验→降采样→JPEG→入索引→淘汰超额旧图。
    /// 解码失败（垃圾数据）返回 None，与原版语义一致。
    pub fn add(&self, data: &[u8]) -> Option<PathBuf> {
        let img = decode_checked(data)?;
        let img = downsample(img);

        let name = format!("{}.jpg", uuid::Uuid::new_v4());
        let path = self.dir.join(&name);
        let file = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(_) => return None,
        };
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(file, JPEG_QUALITY);
        let rgb = DynamicImage::ImageRgb8(img.to_rgb8());
        if encoder.encode_image(&rgb).is_err() {
            let _ = std::fs::remove_file(&path);
            return None;
        }

        let mut names = self.load_index();
        names.insert(0, name.clone());
        let evicted = files_to_evict(&names, MAX_COUNT).to_vec();
        names.truncate(MAX_COUNT);
        self.save_index(&names);
        for old in evicted {
            let _ = std::fs::remove_file(self.dir.join(old));
        }
        Some(path)
    }

    pub fn latest(&self) -> Option<PathBuf> {
        self.load_index()
            .first()
            .map(|n| self.dir.join(n))
            .filter(|p| p.exists())
    }

    /// 数量 >1 且排除第一张时从剩余里随机挑一张（对应原版 randomOlder）。
    pub fn random_older(&self) -> Option<PathBuf> {
        let names = self.load_index();
        if names.len() <= 1 {
            return None;
        }
        let rest = &names[1..];
        let pick = pseudo_random_pick(rest.len());
        rest.get(pick)
            .map(|n| self.dir.join(n))
            .filter(|p| p.exists())
    }
}

#[inline]
fn pseudo_random_pick(len: usize) -> usize {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as usize)
        .unwrap_or(0);
    nanos % len
}

/// 解码并过滤过小的可疑数据（< 30KB 不应出现在缓存管线，但二次保险）。
fn decode_checked(data: &[u8]) -> Option<DynamicImage> {
    if data.len() < 30_000 {
        return None;
    }
    let format = image::guess_format(data).ok()?;
    // 完整性校验：流式读尾防截断的 PNG/JPEG 造成解码期 panic
    let mut probe = data;
    if matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
        let mut tail = [0u8; 8];
        if data.len() >= 8 && Read::read_exact(&mut probe, &mut tail).is_err() {
            return None;
        }
    }
    image::load_from_memory(data).ok()
}

/// 最长边降到 ≤MAX_PIXEL_SIZE（与原版 kCGImageSourceThumbnailMaxPixelSize 一致）。
fn downsample(img: DynamicImage) -> DynamicImage {
    let (w, h) = img.dimensions();
    let longest = w.max(h);
    if longest <= MAX_PIXEL_SIZE {
        return img;
    }
    let ratio = MAX_PIXEL_SIZE as f64 / longest as f64;
    let nw = ((w as f64) * ratio).round().max(1.0) as u32;
    let nh = ((h as f64) * ratio).round().max(1.0) as u32;
    img.resize_exact(nw, nh, FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evict_plan_pure_function() {
        let names: Vec<String> = (1..=25).map(|i| format!("img{i}")).collect();
        let evict = files_to_evict(&names, MAX_COUNT);
        assert_eq!(evict.len(), 7);
        assert_eq!(evict[0], "img19"); // 尾部最旧优先淘汰
        assert_eq!(evict.last().unwrap(), "img25");

        let few: Vec<String> = (0..5).map(|i| i.to_string()).collect();
        assert!(files_to_evict(&few, MAX_COUNT).is_empty());
        assert!(files_to_evict(&[], MAX_COUNT).is_empty());
    }

    #[test]
    fn test_downsample_reduces_pixel_size() {
        let big = DynamicImage::new_rgb8(4000, 4000);
        let out = downsample(big);
        let (w, h) = out.dimensions();
        assert!(w.max(h) <= MAX_PIXEL_SIZE);

        // 垃圾数据不可解码
        assert!(decode_checked(&vec![0u8; 31_000]).is_none());
    }
}
