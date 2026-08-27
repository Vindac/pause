//! ReminderService.swift 的 Rust 移植：全应用唯一计时真相源。
//!
//! 与原版对齐的核心规则：
//! - `deadline` 一律为绝对时间（epoch 秒）；系统睡眠期间计时循环冻结，
//!   唤醒后的第一次 tick 只会把"一个已过期 deadline"转换为一次提醒，
//!   不产生提醒风暴；
//! - 开启「按真实使用时间计时」时，空闲超过阈值的时段按
//!   `min(idle_seconds, elapsed)` 顺延 deadline（离开/睡眠不计入倒计时）；
//! - 锁屏 / 屏保 / 屏幕睡眠时到期提醒暂缓（is_waiting_for_presentation），
//!   环境恢复后下一次 tick 内弹出；
//! - 所有状态转移集中在本模块 handle_tick 与用户动作方法中；
//!   通过注入时钟与系统活动探针实现完全可测试。

use serde::Serialize;
use std::sync::Arc;

/// epoch 秒；f64 保证亚秒精度在 format/ceil 语义上与原版一致。
pub type EpochSecs = f64;
/// 可注入时钟（FakeClock 测试用）。
pub type Clock = Arc<dyn Fn() -> EpochSecs + Send + Sync>;

const MINUTE: EpochSecs = 60.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "tag", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Phase {
    Working {
        deadline: EpochSecs,
    },
    Snoozing {
        deadline: EpochSecs,
        snooze_count: u32,
    },
    /// auto_break_at 为 None 表示关闭自动休息（纯手动模式）。
    Reminding {
        auto_break_at: Option<EpochSecs>,
    },
    Breaking {
        started_at: EpochSecs,
        duration_secs: EpochSecs,
    },
    Paused,
}

impl Phase {
    pub fn tag(&self) -> &'static str {
        match self {
            Phase::Working { .. } => "working",
            Phase::Snoozing { .. } => "snoozing",
            Phase::Reminding { .. } => "reminding",
            Phase::Breaking { .. } => "breaking",
            Phase::Paused => "paused",
        }
    }

    /// working/snoozing 才有待弹出截止时刻。
    pub fn pending_deadline(&self) -> Option<EpochSecs> {
        match self {
            Phase::Working { deadline } | Phase::Snoozing { deadline, .. } => Some(*deadline),
            _ => None,
        }
    }

    pub fn is_breaking(&self) -> bool {
        matches!(self, Phase::Breaking { .. })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BreakSession {
    pub started_at: EpochSecs,
    pub duration_secs: EpochSecs,
}

impl BreakSession {
    pub fn ends_at(&self) -> EpochSecs {
        self.started_at + self.duration_secs
    }

    pub fn remaining(&self, now: EpochSecs) -> EpochSecs {
        (self.ends_at() - now).max(0.0)
    }

    /// mm:ss：总秒数四舍五入（验收样例 300→"05:00"、283→"04:43"、0→"00:00"）。
    pub fn format(remaining: EpochSecs) -> String {
        let total = remaining.round().max(0.0) as i64;
        let m = total / 60;
        let s = total % 60;
        format!("{m:02}:{s:02}")
    }
}

/// 原协议 SystemActivityProviding：锁屏等演示受阻 + 键鼠空闲秒数。
pub trait SystemActivityProviding: Send + Sync {
    fn is_presentation_blocked(&self) -> bool;
    fn user_idle_seconds(&self) -> EpochSecs;
}

/// No-op 实现：默认未接入平台探针时的占位（永不受阻、永不空闲）。
pub struct AlwaysActive;
impl SystemActivityProviding for AlwaysActive {
    fn is_presentation_blocked(&self) -> bool {
        false
    }
    fn user_idle_seconds(&self) -> EpochSecs {
        0.0
    }
}

/// 提醒音触发钩子（原版 NSSound "Tink"；前端播放内置 chime.wav）。
pub type SoundHook = Arc<dyn Fn() + Send + Sync>;

pub struct ReminderDeps {
    pub settings: crate::settings::SharedSettings,
    pub clock: Clock,
    pub system: Arc<dyn SystemActivityProviding>,
    pub on_sound: Option<SoundHook>,
}

/// 对前端/托盘发布的完整快照。每次有意义的变化都会携带自增 seq 发出事件。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub seq: u64,
    pub phase: Phase,
    pub menu_bar_minutes: Option<i64>,
    pub is_waiting_for_presentation: bool,
    pub is_user_idle: bool,
    // 快照时刻由设置决定的派生量，前端免二次取设置
    pub break_duration_minutes: i64,
    pub snooze_minutes: i64,
    pub auto_start_break: bool,
    pub auto_start_break_delay_seconds: i64,
    pub server_now: EpochSecs,
}

pub struct ReminderService {
    deps: ReminderDeps,

    phase: Phase,
    last_tick_at: EpochSecs,
    menu_bar_minutes: Option<i64>,
    is_waiting_for_presentation: bool,
    is_user_idle: bool,
    snooze_count_this_cycle: u32,
    seq: u64,

    /// 仅变化才发布的钩子（对应原 @Published 只在值变时触发 UI）。
    #[cfg(test)]
    published_menu_minutes: std::sync::Arc<std::sync::Mutex<Vec<Option<i64>>>>,
}

fn next_deadline(now: EpochSecs, interval_minutes: i64) -> EpochSecs {
    now + interval_minutes as EpochSecs * MINUTE
}

/// 空闲顺延纯函数：开启按真实使用计时且已空闲越过阈值时，
/// 本 tick 顺延 min(idle, elapsed) 秒 —— 睡眠 8 小时也只顺延本次 tick 实际流逝。
fn idle_postponement(
    idle_seconds: EpochSecs,
    elapsed: EpochSecs,
    threshold_minutes: i64,
    enabled: bool,
) -> EpochSecs {
    if !enabled || elapsed <= 0.0 || idle_seconds < threshold_minutes as EpochSecs * MINUTE {
        return 0.0;
    }
    idle_seconds.min(elapsed)
}

impl ReminderService {
    pub fn new(deps: ReminderDeps) -> Self {
        let now = (deps.clock)();
        let interval = {
            let s = deps.settings.lock().unwrap();
            s.reminder_interval_minutes
        };
        Self {
            deps,
            phase: Phase::Working {
                deadline: next_deadline(now, interval),
            },
            last_tick_at: now,
            menu_bar_minutes: Some(interval),
            is_waiting_for_presentation: false,
            is_user_idle: false,
            snooze_count_this_cycle: 0,
            seq: 0,
            #[cfg(test)]
            published_menu_minutes: Default::default(),
        }
    }

    pub fn start(&mut self) {}

    fn settings_snapshot(&self) -> crate::settings::Settings {
        self.deps.settings.lock().unwrap().clone()
    }

    // ---------------------------------------------------------------
    // Tick 与转移
    // ---------------------------------------------------------------

    /// 心跳入口（1 秒节拍 + 唤醒补拍都走这里）。now 为 None 时用注入时钟。
    pub fn handle_tick(&mut self, now: Option<EpochSecs>) {
        let now = now.unwrap_or_else(|| (self.deps.clock)());
        let s = self.settings_snapshot();

        let elapsed = now - self.last_tick_at;
        self.last_tick_at = now;

        let idle = self.deps.system.user_idle_seconds();
        let postpone =
            idle_postponement(idle, elapsed, s.idle_threshold_minutes, s.activity_based_timing);
        self.update_idle_flag(idle, &s);

        match self.phase {
            Phase::Working { deadline } | Phase::Snoozing { deadline, .. } => {
                let mut d = deadline;
                if postpone > 0.0 {
                    d += postpone;
                    self.phase = match self.phase {
                        Phase::Working { .. } => Phase::Working { deadline: d },
                        Phase::Snoozing { snooze_count, .. } => {
                            Phase::Snoozing { deadline: d, snooze_count }
                        }
                        _ => unreachable!(),
                    };
                }
                if d <= now {
                    if self.deps.system.is_presentation_blocked() {
                        // 弹了也看不见：暂缓，保持过期 deadline，恢复后下一拍弹
                        self.is_waiting_for_presentation = true;
                    } else {
                        self.fire_reminder(&s);
                    }
                } else {
                    self.is_waiting_for_presentation = false;
                    let mins = ((d - now) / MINUTE).ceil() as i64;
                    self.publish_menu_minutes(mins);
                }
            }
            Phase::Breaking { started_at, duration_secs } => {
                let session = BreakSession { started_at, duration_secs };
                let remaining = session.remaining(now);
                if remaining <= 0.0 {
                    self.restart_work_cycle();
                } else {
                    let mins = (remaining / MINUTE).ceil() as i64;
                    self.publish_menu_minutes(mins);
                }
            }
            Phase::Reminding { auto_break_at } => {
                if let Some(at) = auto_break_at {
                    if now >= at {
                        self.start_break();
                    }
                }
            }
            Phase::Paused => {}
        }
    }

    fn update_idle_flag(&mut self, idle: EpochSecs, s: &crate::settings::Settings) {
        self.is_user_idle =
            s.activity_based_timing && idle >= s.idle_threshold_minutes as EpochSecs * MINUTE;
    }

    fn fire_reminder(&mut self, s: &crate::settings::Settings) {
        self.is_waiting_for_presentation = false;
        let auto = if s.auto_start_break {
            Some((self.deps.clock)() + s.auto_start_break_delay_seconds as EpochSecs)
        } else {
            None
        };
        if s.sound_enabled {
            if let Some(hook) = &self.deps.on_sound {
                hook();
            }
        }
        self.phase = Phase::Reminding { auto_break_at: auto };
    }

    // ---------------------------------------------------------------
    // 用户动作
    // ---------------------------------------------------------------

    pub fn start_break(&mut self) {
        if self.phase.is_breaking() {
            return; // 幂等保护
        }
        let s = self.settings_snapshot();
        self.phase = Phase::Breaking {
            started_at: (self.deps.clock)(),
            duration_secs: s.break_duration_minutes as EpochSecs * MINUTE,
        };
    }

    pub fn snooze(&mut self) {
        // 只在提醒中有效：延迟所设分钟数，而非重算完整间隔
        if let Phase::Reminding { .. } = self.phase {
            self.snooze_count_this_cycle += 1;
            let s = self.settings_snapshot();
            self.phase = Phase::Snoozing {
                deadline: (self.deps.clock)() + s.snooze_minutes as EpochSecs * MINUTE,
                snooze_count: self.snooze_count_this_cycle,
            };
        }
    }

    pub fn complete_break(&mut self) {
        self.restart_work_cycle();
    }

    pub fn skip_break(&mut self) {
        self.restart_work_cycle();
    }

    pub fn pause(&mut self) {
        if self.phase == Phase::Paused {
            return;
        }
        self.phase = Phase::Paused;
    }

    pub fn resume(&mut self) {
        if self.phase == Phase::Paused {
            self.restart_work_cycle();
        }
    }

    pub fn restart_work_cycle(&mut self) {
        let interval = self.settings_snapshot().reminder_interval_minutes;
        self.is_waiting_for_presentation = false;
        self.snooze_count_this_cycle = 0;
        self.publish_menu_minutes(interval);
        self.phase = Phase::Working {
            deadline: next_deadline((self.deps.clock)(), interval),
        };
    }

    /// 演示模式：立即进入提醒（PAUSE_DEMO=1 自动流程用）。
    pub fn demo_trigger(&mut self) {
        let s = self.settings_snapshot();
        self.fire_reminder(&s);
    }

    /// 修改间隔设置后调用：从当前时刻重新开始本工作周期
    /// （原版 debounce 300ms 防抖动，由外部设置写入处调度）。
    pub fn on_interval_changed(&mut self) {
        self.restart_work_cycle();
    }

    // ---------------------------------------------------------------
    // 发布
    // ---------------------------------------------------------------

    fn publish_menu_minutes(&mut self, mins: i64) {
        let clamped = mins.max(0);
        #[cfg(test)]
        self.published_menu_minutes.lock().unwrap().push(Some(clamped));
        if self.menu_bar_minutes != Some(clamped) {
            self.menu_bar_minutes = Some(clamped);
            self.seq += 1;
        }
    }

    /// 当前快照（每 tick / 动作后读取并决定是否发事件）。
    pub fn snapshot(&self) -> Snapshot {
        let s = self.settings_snapshot();
        Snapshot {
            seq: self.seq,
            phase: self.phase,
            menu_bar_minutes: self.menu_bar_minutes,
            is_waiting_for_presentation: self.is_waiting_for_presentation,
            is_user_idle: self.is_user_idle,
            break_duration_minutes: s.break_duration_minutes,
            snooze_minutes: s.snooze_minutes,
            auto_start_break: s.auto_start_break,
            auto_start_break_delay_seconds: s.auto_start_break_delay_seconds,
            server_now: (self.deps.clock)(),
        }
    }

    /// 用户动作后强制发一次新版本快照（动作必然改变 UI 关注状态）。
    pub fn snapshot_after_action(&mut self) -> Snapshot {
        self.seq += 1;
        self.snapshot()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn snooze_count_this_cycle(&self) -> u32 {
        self.snooze_count_this_cycle
    }

    #[cfg(test)]
    fn published_log(&self) -> Vec<Option<i64>> {
        self.published_menu_minutes.lock().unwrap().clone()
    }

    /// 测试辅助：先推时钟再执行一拍。
    #[cfg(test)]
    fn handle_tick_at_offset(
        &mut self,
        cell: &std::sync::Arc<std::sync::atomic::AtomicI64>,
        advance: EpochSecs,
    ) {
        cell.fetch_add(advance as i64, std::sync::atomic::Ordering::Relaxed);
        self.handle_tick(None);
    }
}

// 需要跨线程共享给 tokio 任务
type Guarded<M> = std::sync::Arc<std::sync::Mutex<M>>;
pub type SharedReminderService = Guarded<ReminderService>;

// =====================================================================
// 测试：逐条复刻 Tests/PauseTests/ReminderServiceTests.swift（17 例）
// 及相关纯函数/发布用例。
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Settings, SharedSettings};
    use std::sync::atomic::{AtomicI64, Ordering as AtomicOrdering};

    struct FakeClock(Arc<AtomicI64>);
    impl FakeClock {
        fn new(t0: EpochSecs) -> (Self, Arc<AtomicI64>) {
            let cell = Arc::new(AtomicI64::new(t0 as i64));
            (Self(cell.clone()), cell)
        }
        fn advance(&self, secs: EpochSecs) {
            self.0.fetch_add(secs as i64, AtomicOrdering::Relaxed);
        }
        fn get(&self) -> EpochSecs {
            self.0.load(AtomicOrdering::Relaxed) as f64
        }
    }

    #[derive(Default)]
    struct FakeSystemInner {
        blocked: bool,
        idle: EpochSecs,
    }
    #[derive(Clone, Default)]
    struct FakeSystem(Arc<std::sync::Mutex<FakeSystemInner>>);
    impl FakeSystem {
        fn set(&self, blocked: bool, idle: EpochSecs) {
            let mut g = self.0.lock().unwrap();
            g.blocked = blocked;
            g.idle = idle;
        }
    }
    impl SystemActivityProviding for FakeSystem {
        fn is_presentation_blocked(&self) -> bool {
            self.0.lock().unwrap().blocked
        }
        fn user_idle_seconds(&self) -> EpochSecs {
            self.0.lock().unwrap().idle
        }
    }

    struct Fixture {
        service: ReminderService,
        clock: FakeClock,
        cell: Arc<AtomicI64>,
        system: FakeSystem,
    }
    impl Fixture {
        fn clock_cell_get(&self) -> EpochSecs {
            self.cell.load(AtomicOrdering::Relaxed) as f64
        }
    }

    const INTERVAL: i64 = 45;
    const BREAK_MINS: i64 = 5;
    const SNOOZE_MINS: i64 = 5;
    const THRESHOLD: i64 = 2;
    const AUTO_DELAY: i64 = 30;

    fn make_service(extra: impl FnOnce(&mut Settings)) -> Fixture {
        let mut st = Settings {
            reminder_interval_minutes: INTERVAL,
            break_duration_minutes: BREAK_MINS,
            snooze_minutes: SNOOZE_MINS,
            activity_based_timing: true,
            idle_threshold_minutes: THRESHOLD,
            auto_start_break: false,
            auto_start_break_delay_seconds: AUTO_DELAY,
            sound_enabled: false,
            ..Default::default()
        };
        extra(&mut st);
        let shared: SharedSettings = Arc::new(std::sync::Mutex::new(st));
        let t0: EpochSecs = 1_000_000.0;
        let (fake_clock, cell) = FakeClock::new(t0);
        let system = FakeSystem::default();
        let svc = ReminderService::new(ReminderDeps {
            settings: shared,
            clock: {
                let c = cell.clone();
                Arc::new(move || c.load(AtomicOrdering::Relaxed) as f64)
            },
            system: Arc::new(system.clone()),
            on_sound: None,
        });
        Fixture { service: svc, clock: fake_clock, cell, system }
    }

    fn make_defaults() -> Fixture {
        make_service(|_| {})
    }

    // 1 初始 phase
    #[test]
    fn test_initial_phase_is_working_with_interval_deadline() {
        let f = make_defaults();
        match f.service.phase() {
            Phase::Working { deadline } => assert_eq!(deadline, f.clock_cell_get() + 45.0 * 60.0),
            other => panic!("expected working, got {other:?}"),
        }
    }

    // 2 截止前不触发
    #[test]
    fn test_tick_before_deadline_does_not_fire() {
        let mut f = make_defaults();
        f.clock.advance(60.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
        assert!(!f.service.snapshot().is_waiting_for_presentation);
    }

    // 3 到期触发
    #[test]
    fn test_tick_at_deadline_fires_reminder() {
        let mut f = make_defaults();
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Reminding { auto_break_at: None }));
    }

    // 4 受阻暂缓 → 解除后弹出
    #[test]
    fn test_blocked_presentation_defers_reminder() {
        let mut f = make_defaults();
        f.system.set(true, 0.0);
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
        assert!(f.service.snapshot().is_waiting_for_presentation);

        f.system.set(false, 0.0);
        f.clock.advance(2.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Reminding { .. }));
    }

    // 5 延迟只推迟所设分钟
    #[test]
    fn test_snooze_postpones_without_full_interval() {
        let mut f = make_defaults();
        let t0 = f.clock_cell_get();
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None); // 到期进入提醒
        let at_reminder = f.clock_cell_get();
        f.service.snooze();
        match f.service.phase() {
            Phase::Snoozing { deadline, snooze_count: snoozeCount } => {
                // 新 deadline = 当下 + 所设延迟；若误重算完整间隔会是 +45min
                assert_eq!(deadline, at_reminder + SNOOZE_MINS as f64 * 60.0);
                assert!(deadline < t0 + INTERVAL as f64 * 60.0 + INTERVAL as f64 * 60.0);
                assert_eq!(snoozeCount, 1);
            }
            other => panic!("expected snoozing, got {other:?}"),
        }
    }

    // 6 延迟次数不限
    #[test]
    fn test_snooze_is_not_limited() {
        let mut f = make_defaults();
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None); // 首个工作周期到期
        for expected in 1u32..=5 {
            f.service.snooze();
            assert_eq!(f.service.snooze_count_this_cycle(), expected);
            f.clock.advance(SNOOZE_MINS as f64 * 60.0 + 1.0);
            f.service.handle_tick(None); // 再次到期 → reminding
        }
    }

    // 7 新工作周期清零次数
    #[test]
    fn test_snooze_count_resets_after_new_work_cycle() {
        let mut f = make_defaults();
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        f.service.snooze();
        f.clock.advance(SNOOZE_MINS as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        f.service.snooze();
        assert_eq!(f.service.snooze_count_this_cycle(), 2);

        f.service.start_break();
        f.service.skip_break(); // 新周期
        assert_eq!(f.service.snooze_count_this_cycle(), 0);
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
    }

    // 8 休息自然结束从结束时刻起新周期
    #[test]
    fn test_break_completes_and_restarts_work_cycle_from_break_end() {
        let mut f = make_defaults();
        f.service.start_break();
        match f.service.phase() {
            Phase::Breaking { started_at, duration_secs } => {
                assert_eq!(duration_secs, BREAK_MINS as f64 * 60.0);
                f.clock.advance(BREAK_MINS as f64 * 60.0 + 1.0);
                let now = f.clock_cell_get();
                f.service.handle_tick(None);
                match f.service.phase() {
                    Phase::Working { deadline } => {
                        assert_eq!(deadline, now + INTERVAL as f64 * 60.0);
                    }
                    other => panic!("expected working, got {other:?}"),
                }
                let _ = started_at;
            }
            other => panic!("expected breaking, got {other:?}"),
        }
    }

    // 9 跳过立即重启
    #[test]
    fn test_skip_break_restarts_immediately() {
        let mut f = make_defaults();
        f.service.start_break();
        f.service.skip_break();
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
    }

    // 10 暂停/继续
    #[test]
    fn test_pause_and_resume() {
        let mut f = make_defaults();
        f.service.pause();
        f.clock.advance(3600.0 * 10.0);
        f.service.handle_tick(None);
        assert_eq!(f.service.phase(), Phase::Paused);
        f.service.resume();
        match f.service.phase() {
            Phase::Working { deadline } => {
                assert_eq!(deadline, f.clock_cell_get() + INTERVAL as f64 * 60.0)
            }
            other => panic!("expected working, got {other:?}"),
        }
    }

    // 11 睡 8 小时唤醒仅弹一次
    #[test]
    fn test_wake_after_long_sleep_fires_only_one_reminder() {
        let mut f = make_defaults();
        f.system.set(false, 0.0);
        f.clock.advance(3600.0 * 8.0);
        f.service.handle_tick(None); // 单次 tick 消费过期 deadline
        assert!(matches!(f.service.phase(), Phase::Reminding { .. }));

        // 开始休息 → 结束 → 回到 working（无风暴）
        f.service.start_break();
        f.clock.advance(BREAK_MINS as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
    }

    // 12 使用当前间隔重启
    #[test]
    fn test_restart_uses_current_interval() {
        let mut f = make_service(|s| s.reminder_interval_minutes = 30);
        let before = f.clock_cell_get();
        f.service.restart_work_cycle();
        match f.service.phase() {
            Phase::Working { deadline } => assert_eq!(deadline, before + 30.0 * 60.0),
            other => panic!("expected working, got {other:?}"),
        }
    }

    // 13 纯函数 nextDeadline
    #[test]
    fn test_next_deadline_pure_function() {
        assert_eq!(next_deadline(100.0, 10), 700.0);
    }

    // 14 自动开始休息倒计时
    #[test]
    fn test_auto_start_break_begins_break_after_countdown() {
        let mut f = make_service(|s| s.auto_start_break = true);
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        match f.service.phase() {
            Phase::Reminding { auto_break_at: Some(at) } => {
                assert_eq!(at, f.clock_cell_get() + AUTO_DELAY as f64);
            }
            other => panic!("expected auto reminding, got {other:?}"),
        }
        f.clock.advance(29.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Reminding { .. }), "29s 不能提前进休息");
        f.clock.advance(2.0);
        f.service.handle_tick(None);
        assert!(f.service.phase().is_breaking());
    }

    // 15 手动模式永不自动开始
    #[test]
    fn test_manual_mode_never_auto_starts() {
        let mut f = make_defaults(); // autoStart=false
        f.clock.advance(INTERVAL as f64 * 60.0 + 1.0);
        f.service.handle_tick(None);
        match f.service.phase() {
            Phase::Reminding { auto_break_at: None } => {}
            other => panic!("expected manual reminding, got {other:?}"),
        }
        f.clock.advance(3600.0);
        f.service.handle_tick(None);
        assert!(matches!(
            f.service.phase(),
            Phase::Reminding { auto_break_at: None }
        ));
    }

    // 16 空闲顺延 deadline
    #[test]
    fn test_idle_postpones_deadline() {
        let mut f = make_defaults();
        let start = f.clock_cell_get();
        // 先活跃走 10 分钟
        f.clock.advance(600.0);
        f.service.handle_tick(None);
        assert_eq!(f.service.snapshot().menu_bar_minutes, Some(35));

        // 然后离开 8 分钟（>= 2 分钟阈值），墙钟走 6 分钟
        f.system.set(false, 480.0);
        f.clock.advance(360.0);
        f.service.handle_tick(None);
        match f.service.phase() {
            Phase::Working { deadline } => {
                // 顺延 min(480, 360)=360s
                assert!((deadline - (start + 45.0 * 60.0 + 6.0 * 60.0)).abs() <= 2.0);
            }
            other => panic!("expected working, got {other:?}"),
        }
        let snap = f.service.snapshot();
        assert_eq!(snap.menu_bar_minutes, Some(35));
        assert!(snap.is_user_idle);
    }

    // 17 低于阈值不顺延
    #[test]
    fn test_idle_below_threshold_does_not_postpone() {
        let mut f = make_defaults();
        let start = f.clock_cell_get();
        f.system.set(false, 90.0); // < 120s
        f.clock.advance(360.0);
        f.service.handle_tick(None);
        match f.service.phase() {
            Phase::Working { deadline } => assert_eq!(deadline, start + INTERVAL as f64 * 60.0),
            other => panic!("expected working, got {other:?}"),
        }
        assert!(!f.service.snapshot().is_user_idle);
    }

    // 18 功能关闭时即使长空闲也不顺延
    #[test]
    fn test_activity_timing_disabled_does_not_postpone() {
        let mut f = make_service(|s| s.activity_based_timing = false);
        let start = f.clock_cell_get();
        f.system.set(false, 1800.0);
        f.clock.advance(1800.0);
        f.service.handle_tick(None);
        match f.service.phase() {
            Phase::Working { deadline } => assert_eq!(deadline, start + INTERVAL as f64 * 60.0),
            other => panic!("expected working, got {other:?}"),
        }
    }

    // 19 睡眠整段被顺延，唤醒不立刻提醒
    #[test]
    fn test_wake_after_long_idle_does_not_fire_immediately() {
        let mut f = make_defaults();
        // 先活跃走 5 分钟
        f.clock.advance(300.0);
        f.service.handle_tick(None);
        // 模拟睡眠 8 小时（idle=elapsed=28800s）
        f.system.set(false, 28_800.0);
        f.clock.advance(28_800.0);
        f.service.handle_tick(None);
        assert!(matches!(f.service.phase(), Phase::Working { .. }));
        assert_eq!(f.service.snapshot().menu_bar_minutes, Some(40));
    }

    // 20 顺延纯函数四组
    #[test]
    fn test_idle_postponement_pure_function() {
        // disabled → 0
        assert_eq!(idle_postponement(28_800.0, 60.0, THRESHOLD, false), 0.0);
        // idle 100 < 阈值 120，elapsed 60 → 0
        assert_eq!(idle_postponement(100.0, 60.0, THRESHOLD, true), 0.0);
        // idle 600 > 阈值 → min(600, 60) = 60
        assert_eq!(idle_postponement(600.0, 60.0, THRESHOLD, true), 60.0);
        // 整夜睡眠：min(28800, 300) = 300
        assert_eq!(idle_postponement(28_800.0, 300.0, THRESHOLD, true), 300.0);
    }

    // 21 菜单分钟只在变化时发布（ceil 语义）
    #[test]
    fn test_menu_bar_minutes_publishes_minute_values() {
        let f = make_defaults();
        let mut svc = f.service;

        // advance 10s → 剩余 44:50 → ceil = 45
        svc.handle_tick_at_offset(&f.cell, 10.0);
        assert_eq!(svc.published_log().last().copied().flatten(), Some(45));
        // advance 30s（累计 40s）→ 44:20 → 45
        svc.handle_tick_at_offset(&f.cell, 30.0);
        assert_eq!(svc.published_log().last().copied().flatten(), Some(45));
        // advance 60s（累计 100s）→ 43:20 → 44
        svc.handle_tick_at_offset(&f.cell, 60.0);
        assert_eq!(svc.published_log().last().copied().flatten(), Some(44));
    }

    // 26 休息会话 remaining 与 mm:ss 格式化
    #[test]
    fn test_break_session_remaining_and_format() {
        let t0 = 1000.0;
        let session = BreakSession { started_at: t0, duration_secs: 300.0 };
        assert_eq!(session.remaining(t0), 300.0);
        assert!((session.remaining(t0 + 17.0) - 283.0).abs() < f64::EPSILON);
        assert_eq!(session.remaining(t0 + 400.0), 0.0); // 封底
        assert_eq!(BreakSession::format(300.0), "05:00");
        assert_eq!(BreakSession::format(283.0), "04:43");
        assert_eq!(BreakSession::format(0.0), "00:00");
    }
}
