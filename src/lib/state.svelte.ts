/// 与 Rust AppState / Snapshot 对接的全局前端状态。
/// 快照由每秒心跳推送（绝对时间戳，页面按差值自绘秒级文本，切屏不漂移）；
/// 用户动作通过薄命令转发回 Rust 服务层。
import { invoke } from "@tauri-apps/api/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Phase =
  | { tag: "working"; deadline: number }
  | { tag: "snoozing"; deadline: number; snoozeCount: number }
  | { tag: "reminding"; autoBreakAt: number | null }
  | { tag: "breaking"; startedAt: number; durationSecs: number }
  | { tag: "paused" };

export interface Snapshot {
  seq: number;
  phase: Phase;
  menuBarMinutes: number | null;
  isWaitingForPresentation: boolean;
  isUserIdle: boolean;
  breakDurationMinutes: number;
  snoozeMinutes: number;
  autoStartBreak: boolean;
  autoStartBreakDelaySeconds: number;
  serverNow: number;
}

export interface Settings {
  reminderIntervalMinutes: number;
  breakDurationMinutes: number;
  snoozeMinutes: number;
  wallpaperImageURLString: string;
  wallpaperTheme: string;
  launchAtLogin: boolean;
  soundEnabled: boolean;
  overlayOtherWindows: boolean;
  activityBasedTiming: boolean;
  idleThresholdMinutes: number;
  autoStartBreak: boolean;
  autoStartBreakDelaySeconds: number;
  reminderWindowOpacity: number;
  appLanguage: string;
}

type WindowCommand =
  | { command: "show-fade"; opacity: number }
  | { command: "set-opacity"; opacity: number }
  | { command: "hide-fade" };

/** 全局响应式状态（reminder.html 主用）。 */
export const App = $state({
  snap: null as Snapshot | null,
  wallpaperSrc: "",
  /** 窗口内容透明度（CSS 过渡驱动）。 */
  winOpacity: 1,
  /** 窗口显隐淡入淡出状态类。 */
  windowShown: false,
});

let soundUnlisten: (() => void) | undefined;

export async function initReminderState(): Promise<void> {
  const [snap, wallpaperPath] = await Promise.all([
    invoke<Snapshot>("get_snapshot"),
    invoke<string>("get_current_wallpaper"),
  ]);
  applySnapshot(snap);
  setWallpaper(wallpaperPath);

  await listen<Snapshot>("state-changed", (e) => applySnapshot(e.payload));
  await listen<{ path: string }>("wallpaper-changed", (e) =>
    setWallpaper(e.payload.path),
  );
  await listen<WindowCommand>("reminder-window-changed", (e) =>
    applyWindowCommand(e.payload),
  );
  const u = await listen("play-sound", () => playChime());
  soundUnlisten = u;

  // 逐半秒刷新本地时钟派生文本
  setInterval(() => tickNow(), 500);
  App.snap = App.snap; // 触发首次渲染
}

function applySnapshot(snap: Snapshot) {
  // 相变边沿的窗口淡入兜底：若 Rust 端已 show，这里同步 shown 类
  App.snap = snap;
}

function setWallpaper(absPath: string) {
  if (!absPath) return;
  App.wallpaperSrc = convertFileSrc(absPath);
}

function applyWindowCommand(cmd: WindowCommand) {
  switch (cmd.command) {
    case "show-fade":
      App.winOpacity = cmd.opacity;
      App.windowShown = true;
      break;
    case "set-opacity":
      App.winOpacity = cmd.opacity;
      break;
    case "hide-fade":
      App.windowShown = false;
      break;
  }
}

function playChime() {
  try {
    new Audio("/chime.wav").play().catch(() => {});
  } catch {
    /* 音频失败不影响提醒 */
  }
}

// ---------------- 动作 ----------------

export const actions = {
  startBreak: () => invoke("act_start_break"),
  snooze: () => invoke("act_snooze"),
  skipBreak: () => invoke("act_skip_break"),
  pause: () => invoke("act_pause"),
  resume: () => invoke("act_resume"),
};

// ---------------- 本地时钟 ----------------

const listeners = new Set<(nowSecs: number) => void>();

function tickNow() {
  const now = Date.now() / 1000;
  for (const fn of listeners) fn(now);
}

/** 订阅本地逐半秒时钟信号（组件用于重算 mm:ss 等）。返回退订函数。 */
export function onLocalTick(fn: (nowSecs: number) => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

// ---------------- 设置窗使用 ----------------

export async function loadSettings(): Promise<Settings> {
  return invoke<Settings>("get_settings");
}

export async function setSetting(
  key: string,
  value: unknown,
): Promise<Record<string, unknown> | null> {
  const res = await invoke<{ ok: string; value?: Record<string, unknown> }>(
    "set_setting",
    { key, value },
  );
  return res.ok === "ok" ? (res.value ?? null) : res.value ?? null;
}
