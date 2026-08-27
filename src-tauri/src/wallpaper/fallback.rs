//! 兜底渐变风景渲染：WallpaperProvider.swift `BuiltInWallpaperProvider.render(theme:)`
//! 的移植。首次运行 / 无网络时的最终保障 —— 四个具象主题，确定性伪随机
//! （LCG 与 CoreGraphics 版一致），1600×1000。

use crate::settings::WallpaperTheme;
use image::{DynamicImage, Rgba, RgbaImage};

const W: u32 = 1600;
const H: u32 = 1000;

/// 与 Swift 版相同的确定性伪随机：seed &* A &+ B 后右移 33 位。
fn lcg_next(seed: &mut u64) -> u64 {
    *seed = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(14_426_950_408_889_634_07);
    *seed >> 33
}

#[derive(Clone, Copy)]
struct Rgb([f32; 3]);

fn canvas() -> RgbaImage {
    RgbaImage::from_pixel(W, H, Rgba([255, 255, 255, 255]))
}

/// 混色写入（src over dst）。
fn blend(img: &mut RgbaImage, x: i64, y: i64, rgba: [f32; 4]) {
    if x < 0 || y < 0 || x >= W as i64 || y >= H as i64 {
        return;
    }
    let px = img.get_pixel_mut(x as u32, y as u32);
    let d = px.0;
    let sa = rgba[3];
    let out = [
        (rgba[0] * sa + d[0] as f32 / 255.0 * (1.0 - sa)) * 255.0,
        (rgba[1] * sa + d[1] as f32 / 255.0 * (1.0 - sa)) * 255.0,
        (rgba[2] * sa + d[2] as f32 / 255.0 * (1.0 - sa)) * 255.0,
        255.0,
    ];
    *px = Rgba([out[0].round() as u8, out[1].round() as u8, out[2].round() as u8, 255]);
}

fn rgb_at(img: &RgbaImage, x: u32, y: u32) -> Rgb {
    let p = img.get_pixel(x, y).0;
    Rgb([p[0] as f32 / 255.0, p[1] as f32 / 255.0, p[2] as f32 / 255.0])
}

fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    Rgb([
        a.0[0] + (b.0[0] - a.0[0]) * t,
        a.0[1] + (b.0[1] - a.0[1]) * t,
        a.0[2] + (b.0[2] - a.0[2]) * t,
    ])
}

/// 垂直渐变：stops 以「距底比例」给值（CG 坐标习惯），如
/// [(0.0, 底色), (0.35, 中间色), (1.0, 顶色)]；区段外平延端点色。
fn vgrad_vertical(img: &mut RgbaImage, stops: &[(f32, Rgb)]) {
    for y in 0..H {
        let cg_y = 1.0 - y as f32 / H as f32; // 距底比例：0=底 1=顶
        let mut col = find_outer(stops, cg_y);
        for wd in stops.windows(2) {
            let (p0, c0) = wd[0];
            let (p1, c1) = wd[1];
            if cg_y >= p0 && cg_y <= p1 {
                let t = (cg_y - p0) / (p1 - p0).max(f32::EPSILON);
                col = mix(c0, c1, t);
                break;
            }
        }
        for x in 0..W {
            blend(img, x as i64, y as i64, [col.0[0], col.0[1], col.0[2], 1.0]);
        }
    }
}

fn find_outer(stops: &[(f32, Rgb)], cg_y: f32) -> Rgb {
    if cg_y <= stops.first().map(|s| s.0).unwrap_or(0.0) {
        stops.first().map(|s| s.1).unwrap()
    } else {
        stops.last().map(|s| s.1).unwrap()
    }
}

fn fill_circle_blended(img: &mut RgbaImage, cx: f32, cy: f32, r: f32, rgba: [f32; 4]) {
    let y_tl = H as f32 - cy; // 距底坐标 → 左上坐标
    for yy in ((y_tl - r).floor() as i64)..=((y_tl + r).ceil() as i64) {
        for xx in ((cx - r).floor() as i64)..=((cx + r).ceil() as i64) {
            let dx = xx as f32 - cx;
            let dy = yy as f32 - y_tl;
            if dx * dx + dy * dy <= r * r {
                blend(img, xx, yy, rgba);
            }
        }
    }
}

/// 折线山体：步进生成顶点并向下填充至地平线以下整块区域。
fn mountain_layer(
    img: &mut RgbaImage,
    seed: &mut u64,
    base_y_cg: f32,
    amplitude: f32,
    step_min: u32,
    step_span: u32,
    color: Rgb,
) {
    let mut xs: Vec<f32> = Vec::new();
    let mut ys: Vec<f32> = Vec::new();
    let mut x = 0u32;
    while x <= W {
        let r = (lcg_next(seed) % 1000) as f32 / 1000.0;
        let y_cg = base_y_cg + (r - 0.5) * amplitude; // ±amplitude/2
        xs.push(x as f32);
        ys.push(H as f32 - y_cg * H as f32); // TL y
        x += step_min + (lcg_next(seed) % (step_span.max(1)) as u64) as u32;
    }
    for seg in ys.windows(2) {
        let _ = seg; // 折线由逐列采样代替精确连线（视觉等价）
    }
    let last_x = *xs.last().unwrap();
    let n = xs.len();
    for xx in 0..W {
        let xf = xx as f32;
        let t = xf / last_x.max(1.0) * (n.saturating_sub(1)) as f32;
        let i = t.floor() as usize;
        let frac = t - i as f32;
        let j = (i + 1).min(n - 1);
        let ridge = ys[i.min(n - 1)] * (1.0 - frac) + ys[j] * frac;
        for yy in ridge as i64..H as i64 {
            blend(img, xx as i64, yy, [color.0[0], color.0[1], color.0[2], 1.0]);
        }
    }
}

// ---------------------------------------------------------------------
// 四个主题
// ---------------------------------------------------------------------

pub fn render(theme: WallpaperTheme) -> DynamicImage {
    let mut img = canvas();
    match theme {
        WallpaperTheme::Nature => render_nature(&mut img),
        WallpaperTheme::CityNight => render_city_night(&mut img),
        WallpaperTheme::Minimal => render_minimal(&mut img),
        WallpaperTheme::Cartoon => render_cartoon(&mut img),
    }
    DynamicImage::ImageRgba8(img)
}

fn render_nature(img: &mut RgbaImage) {
    vgrad_vertical(
        img,
        &[
            (0.0, Rgb([0.16, 0.28, 0.33])),
            (0.35, Rgb([0.45, 0.62, 0.62])),
            (1.0, Rgb([0.98, 0.86, 0.70])),
        ],
    );
    // 太阳：x=0.62w，距底 y=0.52h，直径 0.13w，95% 不透明
    fill_circle_blended(
        img,
        0.62 * W as f32,
        0.52 * H as f32,
        0.065 * W as f32,
        [1.0, 0.93, 0.78, 0.95],
    );
    // 两层山
    let mut s1 = 3u64;
    mountain_layer(img, &mut s1, 0.34, 0.16, 140, 120, Rgb([0.22, 0.36, 0.32]));
    let mut s2 = 7u64;
    mountain_layer(img, &mut s2, 0.20, 0.13, 140, 120, Rgb([0.10, 0.20, 0.18]));
    // 湖面：底部 20% 区域整体压暗偏青、越靠底越浓（倒影感）
    for y in 0..H {
        let t = y as f32 / H as f32; // 0=顶
        if t < 0.80 {
            continue;
        }
        let alpha = (t - 0.80) / 0.20 * 0.85;
        for x in 0..W {
            let b = rgb_at(img, x, y);
            let g = Rgb([b.0[0] * 0.72, b.0[1] * 0.86, b.0[2] * 0.92]);
            blend(img, x as i64, y as i64, [g.0[0], g.0[1], g.0[2], alpha]);
        }
    }
}

fn render_city_night(img: &mut RgbaImage) {
    vgrad_vertical(
        img,
        &[
            (0.0, Rgb([0.25, 0.20, 0.28])),
            (0.18, Rgb([0.13, 0.16, 0.30])),
            (1.0, Rgb([0.05, 0.07, 0.16])),
        ],
    );
    let mut seed = 42u64;
    // 140 颗星：alpha 0.65，半径 1–3，分布在天空 0.35h~0.95h（距底）
    for _ in 0..140 {
        let fx = (seed_shapeless(&mut seed) % 1000) as f32 / 1000.0;
        let fy = 0.35 + (seed_shapeless(&mut seed) % 1000) as f32 / 1000.0 * 0.60;
        let rad = 1.0 + (seed_shapeless(&mut seed) % 3) as f32;
        fill_circle_blended(img, fx * W as f32, fy * H as f32, rad, [1.0, 1.0, 1.0, 0.65]);
    }
    // 楼宇剪影：宽 40+rand90、高 h*(0.16+rand30%)、间距 8px
    let mut x = 0i64;
    while x < W as i64 {
        let bw = 40 + (seed_shapeless(&mut seed) % 90) as i64;
        let bh_ratio = 0.16 + (seed_shapeless(&mut seed) % 100) as f32 / 100.0 * 0.30;
        let bh = (bh_ratio * H as f32) as i64;
        for yy in 0..bh {
            for xx in x..(x + bw).min(W as i64) {
                blend(img, xx, yy, [0.04, 0.05, 0.10, 1.0]);
            }
        }
        // 约 2/3 概率两扇亮窗
        if (seed_shapeless(&mut seed) % 3) != 0 {
            for win in 0..2 {
                let wx = x + 6 + win * (bw / 2);
                let wy = (bh / 2) + win * 12;
                if wx + 4 < W as i64 && wy + 6 < bh {
                    for dy in 0..6i64 {
                        for dx in 0..4i64 {
                            blend(img, wx + dx, wy + dy, [1.0, 0.85, 0.55, 0.85]);
                        }
                    }
                }
            }
        }
        x += bw + 8;
    }
}

fn seed_shapeless(seed: &mut u64) -> u64 {
    lcg_next(seed)
}

fn render_minimal(img: &mut RgbaImage) {
    // 对角渐变 (0,h)→(w,0)：t = (nx - ny + 1)/2
    for y in 0..H {
        let ny = y as f32 / H as f32;
        for x in 0..W {
            let nx = x as f32 / W as f32;
            let t = ((nx - ny + 1.0) / 2.0).clamp(0.0, 1.0);
            let c = mix(Rgb([0.87, 0.90, 0.86]), Rgb([0.62, 0.73, 0.66]), t);
            blend(img, x as i64, y as i64, [c.0[0], c.0[1], c.0[2], 1.0]);
        }
    }
    // 绿色半透明装饰圆：心(0.55w, 距底0.42h) 直径 0.30w
    fill_circle_blended(
        img,
        0.55 * W as f32,
        0.42 * H as f32,
        0.15 * W as f32,
        [0.36, 0.55, 0.45, 0.55],
    );
    // 米黄圆：心(0.18w, 距底 0.18h) 直径 0.16w
    fill_circle_blended(
        img,
        0.18 * W as f32,
        0.18 * H as f32,
        0.08 * W as f32,
        [0.91, 0.86, 0.74, 0.8],
    );
}

fn render_cartoon(img: &mut RgbaImage) {
    // 随机种子三选一场景（白天草甸 / 黄昏山谷 / 星空夜晚）
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed_base = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos() as u64 % 1_000_000)
        .unwrap_or(7);
    match (seed_base % 3) as usize {
        0 => cartoon_day(img),
        1 => cartoon_dusk(img),
        _ => cartoon_night(img),
    }
}

fn cartoon_day(img: &mut RgbaImage) {
    vgrad_vertical(
        img,
        &[
            (0.55, Rgb([0.98, 0.98, 0.92])),
            (1.0, Rgb([0.45, 0.75, 0.95])),
        ],
    );
    // 太阳光晕 + 本体
    fill_circle_blended(img, 0.75 * W as f32, 0.78 * H as f32, 150.0, [1.0, 0.97, 0.80, 0.5]);
    fill_circle_blended(img, 0.75 * W as f32, 0.78 * H as f32, 65.0, [1.0, 0.93, 0.55, 1.0]);
    // 云（3 组，每组主椭圆）
    let mut seed = 11u64;
    for i in 0..3 {
        let cx = (0.12 + i as f32 * 0.3) * W as f32;
        let cy = (0.68 + (seed_shapeless(&mut seed) % 12) as f32 / 100.0) * H as f32;
        fill_circle_blended(img, cx, cy, 46.0, [1.0, 1.0, 1.0, 0.95]);
        fill_circle_blended(img, cx + 52.0, cy + 8.0, 34.0, [1.0, 1.0, 1.0, 0.95]);
    }
    // 圆顶青山
    let mut s = 23u64;
    mountain_layer(img, &mut s, 0.32, 0.10, 200, 60, Rgb([0.44, 0.66, 0.50]));
    // 草坡前景
    mountain_layer(img, &mut s, 0.14, 0.05, 240, 60, Rgb([0.38, 0.62, 0.36]));
    // 小花点缀
    for _ in 0..8 {
        let fx = (seed_shapeless(&mut seed) % 1000) as f32 / 1000.0;
        let fy = (seed_shapeless(&mut seed) % 100) as f32 / 1000.0;
        fill_circle_blended(img, fx * W as f32, fy * H as f32, 4.0, [0.95, 0.75, 0.83, 0.95]);
    }
}

fn cartoon_dusk(img: &mut RgbaImage) {
    vgrad_vertical(
        img,
        &[
            (0.0, Rgb([0.30, 0.16, 0.34])),
            (0.45, Rgb([0.86, 0.48, 0.36])),
            (1.0, Rgb([0.99, 0.76, 0.44])),
        ],
    );
    // 半沉大太阳：光晕 170px + 本体 65px，位于地平线上
    fill_circle_blended(img, 0.5 * W as f32, H as f32 * 0.5, 170.0, [1.0, 0.85, 0.60, 0.45]);
    fill_circle_blended(img, 0.5 * W as f32, H as f32 * 0.5, 65.0, [1.0, 0.96, 0.82, 1.0]);
    // 三层三角山系（远→近）
    let layers = [
        ([0.45, 0.33, 0.47], 0.38, 0.16),
        ([0.30, 0.22, 0.38], 0.26, 0.13),
        ([0.18, 0.12, 0.26], 0.16, 0.09),
    ];
    for (color, base, amp) in layers {
        let mut s = (amp * 100.0) as u64 + 41;
        mountain_layer(
            img,
            &mut s,
            base,
            amp,
            260,
            120,
            Rgb(color),
        );
    }
}

fn cartoon_night(img: &mut RgbaImage) {
    vgrad_vertical(
        img,
        &[
            (0.0, Rgb([0.08, 0.09, 0.20])),
            (1.0, Rgb([0.03, 0.04, 0.10])),
        ],
    );
    let mut seed = 91u64;
    // 150 颗星
    for _ in 0..150 {
        let fx = (seed_shapeless(&mut seed) % 1000) as f32 / 1000.0;
        let fy = 0.10 + (seed_shapeless(&mut seed) % 800) as f32 / 1000.0;
        fill_circle_blended(
            img,
            fx * W as f32,
            fy * H as f32,
            1.0 + (seed_shapeless(&mut seed) % 2) as f32,
            [1.0, 1.0, 0.94, 0.85],
        );
    }
    // 月牙：亮月本体 + 上弦遮罩（天空色偏移圆）
    let mx = 0.72 * W as f32;
    let my_tl = 0.22 * H as f32;
    fill_circle_blended(img, mx, my_tl, 58.0, [0.98, 0.97, 0.88, 1.0]);
    fill_circle_blended(img, mx - 26.0, my_tl - 14.0, 52.0, [0.05, 0.07, 0.16, 1.0]);
    // 山影多边形 + 小屋 + 暖窗
    let mut s = 61u64;
    mountain_layer(img, &mut s, 0.28, 0.12, 220, 80, Rgb([0.02, 0.03, 0.08]));
    let hx = (0.5 * W as f32) as i64;
    let hy = H as i64 - 130;
    for dy in 0..95i64 {
        for dx in -67i64..67 {
            // 屋身三角切角
            let roof_ok = dy > 60 || dx.abs() < 67 - (dy.max(0)).min(60) / 2;
            if !roof_ok {
                continue;
            }
            blend(img, hx + dx, hy + dy, [0.06, 0.07, 0.12, 1.0]);
        }
    }
    for dy in 0..34i64 {
        for dx in 0..34i64 {
            blend(img, hx + 16 + dx, hy + 20 + dy, [1.0, 0.80, 0.42, 0.95]);
        }
    }
    // 萤火虫 15 只
    for _ in 0..15 {
        let fx = (seed_shapeless(&mut seed) % 1000) as f32 / 1000.0;
        let fy = (seed_shapeless(&mut seed) % 400) as f32 / 1000.0;
        fill_circle_blended(img, fx * W as f32, fy * H as f32, 2.5, [1.0, 0.95, 0.55, 0.9]);
    }
}
