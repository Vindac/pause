//! Windows 平台实现：键鼠空闲（GetLastInputInfo）、锁屏（WTS 会话通知）、
//! 系统睡眠/唤醒（WM_POWERBROADCAST）、屏保轮询、多屏几何（EnumDisplayMonitors
//! + 工作区 rcWork + 每 DPI 缩放换算逻辑坐标）。
//!
//! 说明：显示器关闭在 Windows 上无可靠进程内事件，由"空闲即顺延"的
//! 计时规则兜底（息屏期间必然无输入），行为与原版一致。

use super::{ActivityFlags, SessionEvent};
use std::sync::OnceLock;
use tokio::sync::mpsc::UnboundedSender;

type SharedFlags = Arc<ActivityFlags>;
type Sink = UnboundedSender<SessionEvent>;

use std::sync::Arc;
use windows::core::w;
use windows::Win32::Foundation::{BOOL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{
    WTSRegisterSessionNotification, WTSUnRegisterSessionNotification,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetCursorPos,
    SystemParametersInfoW, TranslateMessage, CW_USEDEFAULT, HWND_MESSAGE, MSG, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASSW,
};

// =====================================================================
// 键鼠空闲
// =====================================================================

/// GetTickCount64 - 上次输入时刻（毫秒差），零权限读取。
pub fn user_idle_seconds() -> f64 {
    unsafe {
        let mut info = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        if GetLastInputInfo(&mut info).as_bool() {
            let now = GetTickCount64();
            let last = info.dwTime as u64;
            let diff_ms = now.checked_sub(last).unwrap_or(0);
            return diff_ms as f64 / 1000.0;
        }
        0.0
    }
}

// =====================================================================
// 会话监听线程：隐藏 message-only 窗口接收 WTS 与电源广播
// =====================================================================

/// WM_WTSSESSION_CHANGE（注册表常量散落，进程内固化值）
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WTS_SESSION_LOCK: u32 = 0x7;
const WTS_SESSION_UNLOCK: u32 = 0x8;
/// WM_POWERBROADCAST
const WM_POWERBROADCAST: u32 = 0x0218;
const PBT_APMRESUMESUSPEND: u32 = 6;
const PBT_APMRESUMEAUTOMATIC: u32 = 7;

static SESSION_FLAGS: OnceLock<SharedFlags> = OnceLock::new();
static SESSION_SINK: OnceLock<Sink> = OnceLock::new();

pub fn start_session_watch(flags: SharedFlags, sink: Sink) {
    // wndproc 是系统回调，无法携带上下文 —— 通过进程级单例桥接
    let _ = SESSION_FLAGS.set(flags);
    let _ = SESSION_SINK.set(sink);

    std::thread::Builder::new()
        .name("pause-session-watch".into())
        .spawn(|| unsafe { run_message_window() })
        .expect("spawn session watch thread");
}

unsafe fn run_message_window() {
    let hinstance: HINSTANCE = GetModuleHandleW(None)
        .expect("module handle")
        .into();
    let class_name = w!("PauseSessionWatch");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(session_wndproc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };
    let atom = RegisterClassW(&wc);
    debug_assert_ne!(atom, 0);

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        class_name,
        w!(""),
        WINDOW_STYLE(0),
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        Some(HWND_MESSAGE),
        None,
        Some(hinstance.into()),
        None,
    )
    .expect("create message-only window");

    // 注册会话锁定通知（NOTIFY_FOR_THIS_SESSION == 0）
    let _ = WTSRegisterSessionNotification(hwnd, 0);

    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    let _ = WTSUnRegisterSessionNotification(hwnd);
}

unsafe extern "system" fn session_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let flags = || SESSION_FLAGS.get().cloned();
    match msg {
        WM_WTSSESSION_CHANGE => match wparam.0 as u32 {
            WTS_SESSION_LOCK => {
                if let Some(f) = flags() {
                    f.set_locked(true);
                    crate::debug_log::debug_log("session locked");
                }
            }
            WTS_SESSION_UNLOCK => {
                if let Some(f) = flags() {
                    f.set_locked(false);
                    crate::debug_log::debug_log("session unlocked");
                }
                if let Some(s) = SESSION_SINK.get() {
                    let _ = s.send(SessionEvent::Unlocked);
                }
            }
            _ => {}
        },
        WM_POWERBROADCAST => match wparam.0 as u32 {
            PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND => {
                crate::debug_log::debug_log("system resumed");
                if let Some(s) = SESSION_SINK.get() {
                    let _ = s.send(SessionEvent::SystemWake);
                }
                LRESULT(1)
            }
            _ => LRESULT(1),
        },
        _ => return DefWindowProcW(hwnd, msg, wparam, lparam),
    }
    LRESULT(0)
}

/// 屏保运行状态每秒轮询（SPI_GETSCREENSAVERRUNNING）。
const SPI_GETSCREENSAVERRUNNING: u32 = 0x0072;

pub fn poll_screensaver(flags: &ActivityFlags) {
    unsafe {
        let mut running: BOOL_LOCAL = BOOL_LOCAL(0);
        let ok = SystemParametersInfoW(
            SPI_GETSCREENSAVERRUNNING,
            0,
            Some((&mut running.0 as *mut i32).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .as_bool();
        flags.set_saver(ok && running.0 != 0);
    }
}
#[repr(C)]
struct BOOL_LOCAL(i32);

// =====================================================================
// 多屏几何（物理像素 → 逻辑点）
// =====================================================================

#[derive(Debug, Clone, Copy)]
pub struct RectL {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl RectL {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

#[derive(Debug, Clone, Copy)]
struct ScreenPhys {
    monitor: RECT,
    work: RECT,
    dpi_scale: f64,
}

impl ScreenPhys {
    fn logical_frame(&self) -> RectL {
        phys_to_logical_rect(self.monitor, self.dpi_scale)
    }
    fn logical_work(&self) -> RectL {
        phys_to_logical_rect(self.work, self.dpi_scale)
    }
}

fn phys_to_logical_rect(r: RECT, scale: f64) -> RectL {
    RectL {
        x: r.left as f64 / scale,
        y: r.top as f64 / scale,
        w: (r.right - r.left) as f64 / scale,
        h: (r.bottom - r.top) as f64 / scale,
    }
}

unsafe extern "system" fn monitor_enum_proc(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let list = &mut *(lparam.0 as *mut Vec<HMONITOR>);
    list.push(hmon);
    BOOL(1)
}

fn enumerate_screens() -> Vec<ScreenPhys> {
    let mut monitors: Vec<HMONITOR> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        );
    }
    monitors
        .into_iter()
        .filter_map(|hmon| {
            let mut mi = MONITORINFOEXW::default();
            mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
            let ok = unsafe {
                GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFOEXW as *mut MONITORINFO)
            };
            if !ok.as_bool() {
                return None;
            }
            let mut dpi_x: u32 = 96;
            let mut dpi_y: u32 = 96;
            unsafe {
                let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
            }
            let dpi = if dpi_x >= 96 { dpi_x } else { 96 };
            Some(ScreenPhys {
                monitor: mi.monitorInfo.rcMonitor,
                work: mi.monitorInfo.rcWork,
                dpi_scale: dpi as f64 / 96.0,
            })
        })
        .collect()
}

pub struct ScreenInfoL {
    pub frame: RectL,
    pub work_area: RectL,
}

pub fn list_screens() -> Vec<ScreenInfoL> {
    enumerate_screens()
        .iter()
        .map(|s| ScreenInfoL {
            frame: s.logical_frame(),
            work_area: s.logical_work(),
        })
        .collect()
}

pub fn cursor_position() -> Option<(f64, f64)> {
    let mut pt = POINT::default();
    unsafe {
        GetCursorPos(&mut pt).ok()?;
    }
    // 找到包含鼠标物理点的显示器，按其 DPI 换算逻辑坐标
    let screens = enumerate_screens();
    for s in &screens {
        if (pt.x as f64) >= s.monitor.left as f64
            && (pt.x as f64) < s.monitor.right as f64
            && (pt.y as f64) >= s.monitor.top as f64
            && (pt.y as f64) < s.monitor.bottom as f64
        {
            return Some((
                pt.x as f64 / s.dpi_scale,
                pt.y as f64 / s.dpi_scale,
            ));
        }
    }
    // 找不到所属屏（罕见）：退化为全部按主屏缩放
    screens.first().map(|s| {
        (
            pt.x as f64 / s.dpi_scale,
            pt.y as f64 / s.dpi_scale,
        )
    })
}
