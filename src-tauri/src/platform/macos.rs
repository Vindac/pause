//! macOS 平台实现：键鼠空闲（CGEventSource）、锁屏/屏保/睡眠监听
//! （NSDistributedNotificationCenter + NSWorkspace）、多屏几何（NSScreen）。
//!
//! AppKit/Foundation 类均通过 runtime 消息发送调用，坐标统一换算为
//! 「左上原点全局逻辑点」空间，与 Tauri LogicalPosition 语义一致。

use super::{ActivityFlags, SessionEvent};
use objc2::{class, msg_send, runtime::AnyObject};
use objc2_foundation::{NSPoint, NSRect};
use objc2_foundation::NSString;
use block2::RcBlock;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

type SharedFlags = Arc<ActivityFlags>;
type Sink = UnboundedSender<SessionEvent>;

// =====================================================================
// 键鼠空闲：CGEventSource.secondsSinceLastEventType(.combinedSessionState,
// eventType: kCGAnyInputEventType)，只读事件时间戳，无需辅助功能权限。
// =====================================================================

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state_id: i32, event_type: u32) -> f64;
}

/// kCGEventSourceStateCombinedSessionState == -1；kCGAnyInputEventType == ~0。
pub fn user_idle_seconds() -> f64 {
    let secs = unsafe { CGEventSourceSecondsSinceLastEventType(-1, u32::MAX) };
    // 非有限或负数按 0（活跃）处理，与原版一致
    if secs.is_finite() && secs >= 0.0 {
        secs
    } else {
        0.0
    }
}

// =====================================================================
// 锁屏 / 屏保 / 屏幕睡眠 / 唤醒 监听
// =====================================================================

pub fn start_session_watch(flags: SharedFlags, sink: Sink) {
    std::thread::Builder::new()
        .name("pause-session-watch".into())
        .spawn(move || unsafe { run_observer_loop(flags, sink) })
        .expect("spawn session watch thread");
}

unsafe fn run_observer_loop(flags: SharedFlags, sink: Sink) {
    use objc2::class;

    let dist_center: *mut AnyObject =
        msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
    let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
    let ws_center: *mut AnyObject = msg_send![workspace, notificationCenter];

    // 锁屏
    let fl_locked = flags.clone();
    observe(dist_center, "com.apple.screenIsLocked", move || {
        fl_locked.set_locked(true);
        crate::debug_log::debug_log("screen locked");
    });
    let (fl, sk) = (flags.clone(), sink.clone());
    observe(dist_center, "com.apple.screenIsUnlocked", move || {
        fl.set_locked(false);
        crate::debug_log::debug_log("screen unlocked");
        let _ = sk.send(SessionEvent::Unlocked);
    });

    // 屏保 开始/结束
    let fl2 = flags.clone();
    observe(dist_center, "com.apple.screensaver.didstart", move || {
        fl2.set_saver(true);
        crate::debug_log::debug_log("screensaver started");
    });
    let fl3 = flags.clone();
    observe(dist_center, "com.apple.screensaver.didstop", move || {
        fl3.set_saver(false);
        crate::debug_log::debug_log("screensaver stopped");
    });

    // 显示器睡眠/唤醒（NSWorkspace 通知中心）
    let (fl4, _sk4) = (flags.clone(), sink.clone());
    observe(
        ws_center,
        "NSWorkspaceScreensDidSleepNotification",
        move || {
            fl4.set_asleep(true);
            crate::debug_log::debug_log("screens did sleep");
        },
    );
    let (fl5, sk5) = (flags.clone(), sink.clone());
    observe(ws_center, "NSWorkspaceScreensDidWakeNotification", move || {
        fl5.set_asleep(false);
        crate::debug_log::debug_log("screens did wake");
        let _ = sk5.send(SessionEvent::ScreensAwake);
    });
    // 系统唤醒后立即补一拍（防提醒风暴的另一半）
    let sk6 = sink.clone();
    observe(ws_center, "NSWorkspaceDidWakeNotification", move || {
        crate::debug_log::debug_log("system did wake");
        let _ = sk6.send(SessionEvent::SystemWake);
    });

    // 永久消费 runloop；observer 在进程生命周期内有效（与原版 deinit 语义等同）
    let run_loop: *mut AnyObject = msg_send![class!(NSRunLoop), currentRunLoop];
    loop {
        let mode = NSString::from_str("kCFRunLoopDefaultMode");
        let distant_past: *mut AnyObject = msg_send![class!(NSDate), distantPast];
        let _: i8 = msg_send![run_loop, runMode: &*mode, beforeDate: distant_past];
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// 注册一个通知观察者（block 式回调，在发布线程执行）。
unsafe fn observe(center: *mut AnyObject, name: &str, action: impl Fn() + Send + 'static) {
    let ns_name = NSString::from_str(name);
    let action = Arc::new(action);
    let block = RcBlock::new(move |_note: NonNull<AnyObject>| {
        action();
    });
    let _: *mut AnyObject = msg_send![
        center,
        addObserverForName: &*ns_name,
        object: std::ptr::null_mut::<AnyObject>(),
        queue: std::ptr::null_mut::<AnyObject>(),
        usingBlock: &*block
    ];
}

// =====================================================================
// 多屏几何：frame / visibleFrame → 全局左上逻辑点；鼠标位置 NSEvent
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
pub struct ScreenInfoL {
    pub frame: RectL,
    pub work_area: RectL,
}

fn flip_to_top_left(rect: NSRect, primary_max_y: f64) -> RectL {
    RectL {
        x: rect.origin.x,
        y: primary_max_y - rect.origin.y - rect.size.height,
        w: rect.size.width,
        h: rect.size.height,
    }
}

unsafe fn screen_frame(screen: *mut AnyObject) -> NSRect {
    msg_send![screen, frame]
}

pub fn list_screens() -> Vec<ScreenInfoL> {
    unsafe {
        let main_screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        if main_screen.is_null() {
            return Vec::new();
        }
        let mf = screen_frame(main_screen);
        let primary_max_y = mf.origin.y + mf.size.height;

        let arr: *mut AnyObject = msg_send![class!(NSScreen), screens];
        if arr.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![arr, count];
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let s: *mut AnyObject = msg_send![arr, objectAtIndexedSubscript: i];
            let f = screen_frame(s);
            let v: NSRect = msg_send![s, visibleFrame];
            out.push(ScreenInfoL {
                frame: flip_to_top_left(f, primary_max_y),
                work_area: flip_to_top_left(v, primary_max_y),
            });
        }
        out
    }
}

/// 鼠标所在位置的全局左上逻辑点（NSEvent.mouseLocation 为左下原点）。
pub fn cursor_position() -> Option<(f64, f64)> {
    unsafe {
        let main_screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        if main_screen.is_null() {
            return None;
        }
        let mf = screen_frame(main_screen);
        let primary_max_y = mf.origin.y + mf.size.height;
        let p: NSPoint = msg_send![class!(NSEvent), mouseLocation];
        Some((p.x, primary_max_y - p.y))
    }
}

// 引用计数守卫：msg_send 返回的对象遵循 macOS autoreleased 约定，
// observer runloop 线程内无需显式 retain/release。
#[allow(dead_code)]
static _RUNLOOP_HEARTBEAT: AtomicI64 = AtomicI64::new(0);

/// 对调试日志暴露当前锁屏等状态（供 demo 排查）。
pub fn flags_snapshot(flags: &ActivityFlags) -> String {
    format!(
        "locked={} saver={} asleep={}",
        flags.screen_locked.load(AtomicOrdering::Relaxed),
        flags.screensaver_active.load(AtomicOrdering::Relaxed),
        flags.screen_asleep.load(AtomicOrdering::Relaxed)
    )
}
