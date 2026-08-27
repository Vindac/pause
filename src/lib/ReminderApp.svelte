<script lang="ts">
  // 提醒窗两页容器：提醒页 ↔ 休息页同窗常驻 opacity 切换，
  // 保证窗口淡出时内容不闪空（复刻 ReminderContainerView）。
  import { onMount } from "svelte";
  import { t, initI18n } from "./i18n.svelte";
  import {
    App,
    actions,
    initReminderState,
    onLocalTick,
    type Phase,
    type Snapshot,
  } from "./state.svelte";

  let nowSecs = $state(0);
  let prevWallpaper = $state("");
  let wallpaperAVisible = $state(true);
  let imgASrc = $state("");
  let imgBSrc = $state("");

  onMount(() => {
    // 提醒窗是独立 webview：必须自己拉取字典，否则 t() 回退显示键名（形似英文）
    void initI18n();
    void initReminderState();
    const off = onLocalTick((n) => (nowSecs = n));
    return off;
  });

  // 当前壁纸变化 → 与旧图交叉淡化 0.8s
  let firstRun = true;
  $effect(() => {
    const src = App.wallpaperSrc;
    if (!src) return;
    if (firstRun) {
      firstRun = false;
      imgASrc = src;
      return;
    }
    if (src === (wallpaperAVisible ? imgASrc : imgBSrc)) return;
    prevWallpaper = wallpaperAVisible ? imgASrc : imgBSrc;
    if (wallpaperAVisible) {
      imgBSrc = src;
      wallpaperAVisible = false;
    } else {
      imgASrc = src;
      wallpaperAVisible = true;
    }
  });

  const snap = $derived(App.snap as Snapshot | null);
  const phase: Phase | null = $derived(snap?.phase ?? null);

  const isReminderPage = $derived(phase?.tag === "reminding");
  const isBreakPage = $derived(phase?.tag === "breaking");

  function breakRemaining(p: Phase): number {
    if (p.tag !== "breaking") return 0;
    return Math.max(0, p.startedAt + p.durationSecs - nowSecs);
  }

  function formatMMSS(remaining: number): string {
    const total = Math.max(0, Math.round(remaining));
    const m = Math.floor(total / 60);
    const s = total % 60;
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  }

  // 「开始休息」按钮逐秒文案：自动倒计时剩余秒数向上取整
  const startBreakLabel = $derived.by(() => {
    if (
      phase?.tag === "reminding" &&
      phase.autoBreakAt != null &&
      nowSecs > 0
    ) {
      const remaining = phase.autoBreakAt - nowSecs;
      if (remaining > 0) {
        return t("reminderStartIn", { s: Math.ceil(remaining) });
      }
    }
    return t("reminderStart");
  });

  const breakDurationText = $derived(
    snap ? `${String(snap.breakDurationMinutes).padStart(2, "0")}:00` : "00:00",
  );
</script>

<div class="root" class:shown={App.windowShown} style={`opacity: ${App.windowShown ? App.winOpacity : 0}`}>
  <!-- 壁纸背景：Ken Burns + 交叉淡化 -->
  <div class="backdrop" aria-hidden="true">
    <img
      class="bg kenburns"
      src={imgASrc}
      alt=""
      style="opacity:{wallpaperAVisible ? 1 : 0}"
    />
    <img
      class="bg kenburns"
      src={imgBSrc}
      alt=""
      style="opacity:{wallpaperAVisible ? 0 : 1}"
    />
  </div>
  <div class="readability" aria-hidden="true"></div>

  <!-- ======== 提醒页 ======== -->
  <div class="page reminder-page" class:visible={isReminderPage}>
    <div class="spacer"></div>
    <h1 class="title">{t("reminderTitle")}</h1>
    <p class="subtitle">{t("reminderSubtitle")}</p>
    <div class="spacer"></div>
    <div class="break-duration mono">{t("reminderBreakFor", { t: breakDurationText })}</div>
    <div class="buttons">
      <button class="glass" type="button" onclick={() => actions.snooze()}>
        {t("reminderDelay", { m: snap?.snoozeMinutes ?? "" })}
      </button>
      <button class="glass prominent mono" type="button" onclick={() => actions.startBreak()}>
        {startBreakLabel}
      </button>
    </div>
  </div>

  <!-- ======== 休息页 ======== -->
  <div class="page break-page" class:visible={isBreakPage}>
    <div class="spacer"></div>
    <div class="emoji">🌲</div>
    <h1 class="title small">{t("breakTitle")}</h1>
    <div class="countdown mono">
      {formatMMSS(phase && isBreakPage ? breakRemaining(phase) : 0)}
    </div>
    <p class="hint">
      {phase?.tag === "breaking" && breakRemaining(phase) <= 3
        ? t("breakAlmostDone")
        : t("breakHint")}
    </p>
    <div class="spacer low"></div>
    <button class="glass" type="button" onclick={() => actions.skipBreak()}>
      {t("breakSkip")}
    </button>
  </div>
</div>

<style>
  :global(html, body.reminder-body) {
    margin: 0;
    height: 100%;
    background: transparent;
    overflow: hidden;
  }
  .root {
    position: fixed;
    inset: 0;
    border-radius: 16px;
    overflow: hidden;
    background: transparent;
    opacity: 0;
    transition: opacity 0.35s ease-in-out; /* 淡入 0.35s；淡出由 .fading 变体覆盖 */
    user-select: none;
    cursor: default;
  }
  .root:not(.shown) {
    transition: opacity 0.5s ease-in-out; /* 淡出 0.5s */
    pointer-events: none;
  }

  /* ---------- 壁纸背景 ---------- */
  .backdrop {
    position: absolute;
    inset: 0;
    background: #000;
  }
  .bg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: opacity 0.8s ease-in-out; /* 切图交叉淡化 */
    will-change: transform, opacity;
  }
  .kenburns {
    animation: kb 45s ease-in-out infinite alternate; /* 1.00→1.05 往返 */
  }
  @keyframes kb {
    from { transform: scale(1); }
    to { transform: scale(1.05); }
  }
  /* 底部可读性渐变 */
  .readability {
    position: absolute;
    inset: 0;
    background: linear-gradient(to bottom, rgba(0, 0, 0, 0) 55%, rgba(0, 0, 0, 0.3) 80%, rgba(0, 0, 0, 0.72) 100%);
  }

  /* ---------- 两页叠放 ---------- */
  .page {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    color: #fff;
    opacity: 0;
    transition: opacity 0.4s ease-in-out; /* 两页交叉淡化 */
    pointer-events: none;
  }
  .page.visible {
    opacity: 1;
    pointer-events: auto;
  }
  .spacer { flex: 1; }
  .spacer.low { flex: 0.6; }

  .title {
    margin: 0;
    font-size: 42px;
    font-weight: 700;
    font-family: ui-rounded, -apple-system, system-ui, sans-serif;
    letter-spacing: 0.01em;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
  }
  .title.small {
    font-size: 32px;
    font-weight: 600;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
  }
  .subtitle {
    margin: 10px 0 0;
    font-size: 20px;
    opacity: 0.92;
    text-shadow: 0 1px 6px rgba(0, 0, 0, 0.4);
  }
  .mono {
    font-variant-numeric: tabular-nums;
    font-family: -apple-system, system-ui, sans-serif;
  }

  .break-duration {
    font-size: 17px;
    font-weight: 500;
    opacity: 0.95;
    text-shadow: 0 1px 5px rgba(0, 0, 0, 0.4);
  }
  .buttons {
    display: flex;
    gap: 16px;
    margin-top: 20px;
    padding-bottom: 52px;
  }

  /* ---------- 玻璃胶囊按钮 ---------- */
  .glass {
    appearance: none;
    border: 1px solid rgba(255, 255, 255, 0.35);
    border-radius: 999px;
    color: #fff;
    font-size: 16px;
    padding: 12px 26px;
    background:
      linear-gradient(rgba(255, 255, 255, 0.14), rgba(255, 255, 255, 0.14)),
      rgba(120, 120, 128, 0.24);
    backdrop-filter: blur(28px) saturate(1.7);
    -webkit-backdrop-filter: blur(28px) saturate(1.7);
    cursor: pointer;
    transition: transform 0.12s ease-out, opacity 0.12s ease-out;
  }
  .glass:hover {
    background:
      linear-gradient(rgba(255, 255, 255, 0.2), rgba(255, 255, 255, 0.2)),
      rgba(120, 120, 128, 0.28);
  }
  .glass:active {
    transform: scale(0.97);
    opacity: 0.7;
  }
  .glass.prominent {
    font-weight: 600;
    background:
      linear-gradient(rgba(255, 255, 255, 0.32), rgba(255, 255, 255, 0.32)),
      rgba(120, 120, 128, 0.3);
  }

  /* ---------- 休息页专属 ---------- */
  .emoji {
    font-size: 56px;
    line-height: 1;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.35);
  }
  .break-page .title.small { margin-top: 22px; }
  .countdown {
    font-size: 64px;
    font-weight: 300;
    margin-top: 6px;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.45);
  }
  .hint {
    margin: 14px 0 0;
    font-size: 18px;
    opacity: 0.9;
    text-shadow: 0 1px 5px rgba(0, 0, 0, 0.35);
  }
  .break-page button.glass {
    margin-bottom: 48px;
  }
</style>
