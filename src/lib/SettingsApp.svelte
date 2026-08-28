<script lang="ts">
  // 设置窗：五组控件（通用 / 提醒 / 图片 / 系统 / 提醒窗口），
  // 全部修改即保存；取值经 Rust 钳制后回填。
  import { onMount } from "svelte";
  import { t, L10n, initI18n } from "./i18n.svelte";
  import {
    loadSettings,
    setSetting,
    type Settings,
  } from "./state.svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";

  interface UpdateInfo {
    version: string;
    tag: string;
    url: string;
    notes: string;
    inAppAvailable: boolean;
  }

  let s = $state<Settings | null>(null);
  let version = $state("");
  let wallpaperPreview = $state("");
  let switching = $state(false);
  let customSnoozeText = $state("5");
  let customIntervalText = $state("");
  /** 更新弹窗：发现新版本时非空。 */
  let updateInfo = $state<UpdateInfo | null>(null);
  /** idle / checking / downloading / installing */
  let updatePhase = $state<"idle" | "checking" | "downloading" | "installing">("idle");
  let downloadPct = $state(0);
  /** 手动检查的行内状态："" / uptodate / failed */
  let checkStatus = $state<"" | "uptodate" | "failed">("");

  const QUICK_INTERVALS = [30, 45, 60];
  const IDLE_THRESHOLDS = [1, 2, 3, 5];
  const AUTO_DELAYS = [10, 20, 30, 60];
  const SNOOZE_QUICK = [1, 2, 3, 4, 5];
  const BREAK_DURATIONS = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 30, 60];

  onMount(async () => {
    await initI18n();
    s = await loadSettings();
    version = await invokeVersion();
    customSnoozeText = String(s?.snoozeMinutes ?? 5);
    const p = await invokeCurrentWallpaper();
    if (p) wallpaperPreview = convertFileSrc(p);
    void listen<{ path: string }>("wallpaper-changed", (e) => {
      wallpaperPreview = convertFileSrc(e.payload.path);
    });
    // 启动自检发现新版本 → 直接弹出双通道更新弹窗
    void listen<UpdateInfo>("update-available", (e) => {
      updateInfo = e.payload;
      updatePhase = "idle";
    });
  });

  async function invokeVersion(): Promise<string> {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string>("app_version");
  }
  async function invokeCurrentWallpaper(): Promise<string> {
    const { invoke } = await import("@tauri-apps/api/core");
    return invoke<string>("get_current_wallpaper");
  }

  /** 写入某键并接收钳制后的完整快照回填。 */
  async function put(key: string, value: unknown) {
    const snap = await setSetting(key, value);
    if (snap && s) {
      Object.assign(s, snap as Partial<Settings>);
      syncCustomTexts(s);
    }
  }

  function syncCustomTexts(cur: Settings) {
    customSnoozeText = String(cur.snoozeMinutes);
    customIntervalText = String(cur.reminderIntervalMinutes);
  }

  async function changeLanguage(lang: string) {
    L10n.lang = lang;
    await import("@tauri-apps/api/core").then((c) =>
      c.invoke("set_language", { lang }),
    );
  }

  /** 倒计时选择：任何选项都启用自动开始；「立即」= 0 秒（弹出后马上休息）。 */
  async function setCountdown(v: string) {
    await put("autoStartBreak", true);
    await put("autoStartBreakDelaySeconds", Number(v));
  }

  async function commitCustomSnooze() {
    const parsed = parseInt(customSnoozeText.trim(), 10);
    if (!Number.isFinite(parsed)) {
      customSnoozeText = String(s?.snoozeMinutes ?? 5); // 解析失败回退原值
      return;
    }
    const clamped = Math.min(15, Math.max(1, parsed));
    customSnoozeText = String(clamped);
    await put("snoozeMinutes", clamped);
  }

  async function commitCustomInterval() {
    const parsed = parseInt(customIntervalText.trim(), 10);
    if (!Number.isFinite(parsed)) {
      customIntervalText = String(s?.reminderIntervalMinutes ?? 45);
      return;
    }
    const clamped = Math.min(180, Math.max(10, parsed));
    customIntervalText = String(clamped);
    await put("reminderIntervalMinutes", clamped);
  }

  async function switchWallpaper() {
    if (switching) return;
    switching = true;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const path = await invoke<string>("switch_wallpaper");
      if (path) wallpaperPreview = convertFileSrc(path);
    } finally {
      switching = false;
    }
  }

  // ---------------- 检查更新（双通道） ----------------

  /** 手动检查：Rust 端合并 updater + GitHub API 双通道结果。 */
  async function checkForUpdate() {
    if (updatePhase !== "idle") return;
    updatePhase = "checking";
    checkStatus = "";
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const info = await invoke<UpdateInfo | null>("check_update");
      if (info) {
        updateInfo = info;
      } else {
        checkStatus = "uptodate";
      }
    } catch {
      checkStatus = "failed";
    } finally {
      updatePhase = "idle";
    }
  }

  /** 方案 A：应用内下载 + 安装 + 重启（updater 插件，带签名校验）。 */
  async function installNow() {
    if (!updateInfo || updatePhase !== "idle") return;
    updatePhase = "downloading";
    downloadPct = 0;
    let received = 0;
    let total = 0;
    try {
      const { check } = await import("@tauri-apps/plugin-updater");
      const update = await check();
      if (!update) {
        // 竞态下 latest.json 已指向当前版本 → 无需更新
        updateInfo = null;
        checkStatus = "uptodate";
        return;
      }
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started" && ev.data.contentLength) {
          total = ev.data.contentLength;
        } else if (ev.event === "Progress") {
          received += ev.data.chunkLength;
          if (total > 0) {
            downloadPct = Math.min(100, Math.round((received / total) * 100));
          }
        } else if (ev.event === "Finished") {
          downloadPct = 100;
        }
      });
      updatePhase = "installing";
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    } catch {
      updatePhase = "idle";
      checkStatus = "failed";
    }
  }

  /** 方案 B：跳转 GitHub Release 页面手动下载。 */
  async function openGithubRelease() {
    if (!updateInfo?.url) return;
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(updateInfo.url);
  }

  /** 本版本不再提醒（仅抑制启动自检弹窗）。 */
  async function skipThisVersion() {
    if (updateInfo) {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("skip_update_version", { version: updateInfo.version });
    }
    updateInfo = null;
  }

  const isCustomInterval = $derived(
    !!s && !QUICK_INTERVALS.includes(s.reminderIntervalMinutes),
  );
  const isCustomSnooze = $derived(
    !!s && !SNOOZE_QUICK.includes(s.snoozeMinutes),
  );
</script>

{#if s}
  <div class="form">
    <!-- ============ 通用 ============ -->
    <section>
      <h2>{t("sectionGeneral")}</h2>
      <div class="row">
        <label>{t("languageLabel")}</label>
        <select value={L10n.lang} onchange={(e) => changeLanguage(e.currentTarget.value)}>
          <option value="english">English</option>
          <option value="chinese">简体中文</option>
        </select>
      </div>
      <div class="row">
        <label>{t("versionLabel")}</label>
        <span class="value">v{version}</span>
      </div>
      <div class="row">
        <label>{t("checkUpdate")}</label>
        <span class="value update-row">
          <button
            class="push"
            type="button"
            disabled={updatePhase !== "idle"}
            onclick={checkForUpdate}
          >
            {updatePhase === "checking" ? t("updateChecking") : t("checkUpdate")}
          </button>
          {#if checkStatus === "uptodate"}<span>{t("upToDate")}</span>
          {:else if checkStatus === "failed"}<span>{t("updateCheckFailed")}</span>{/if}
        </span>
      </div>
    </section>

    <!-- ============ 提醒 ============ -->
    <section>
      <h2>{t("sectionReminder")}</h2>
      <div class="row">
        <label>{t("intervalLabel")}</label>
        <select
          value={String(s.reminderIntervalMinutes)}
          onchange={(e) =>
            put("reminderIntervalMinutes", Number(e.currentTarget.value))}
        >
          {#each QUICK_INTERVALS as q}
            <option value={String(q)}>{t("minutes", { m: q })}</option>
          {/each}
          {#if isCustomInterval || !QUICK_INTERVALS.includes(s.reminderIntervalMinutes)}
            <option value={String(s.reminderIntervalMinutes)}>
              {t("customMinutes", { m: s.reminderIntervalMinutes })}
            </option>
          {/if}
        </select>
      </div>
      {#if isCustomInterval}
        <div class="row indent">
          <label>{t("customIntervalText", { m: s.reminderIntervalMinutes })}</label>
          <span class="stepper">
            <button
              aria-label="-"
              disabled={s.reminderIntervalMinutes <= 10}
              onclick={() => put("reminderIntervalMinutes", s.reminderIntervalMinutes - 1)}>−</button
            >
            <input
              class="num"
              bind:value={customIntervalText}
              onchange={commitCustomInterval}
            />
            <button
              aria-label="+"
              disabled={s.reminderIntervalMinutes >= 180}
              onclick={() => put("reminderIntervalMinutes", s.reminderIntervalMinutes + 1)}>+</button
            >
          </span>
        </div>
      {/if}

      <div class="row">
        <label>{t("breakDurationLabel")}</label>
        <select
          value={String(s.breakDurationMinutes)}
          onchange={(e) =>
            put("breakDurationMinutes", Number(e.currentTarget.value))}
        >
          {#each BREAK_DURATIONS as m}
            <option value={String(m)}>{t("minutes", { m })}</option>
          {/each}
          {#if !BREAK_DURATIONS.includes(s.breakDurationMinutes)}
            <option value={String(s.breakDurationMinutes)}>
              {t("customMinutes", { m: s.breakDurationMinutes })}
            </option>
          {/if}
        </select>
      </div>

      <div class="row">
        <label>{t("usageTimingLabel")}</label>
        <input
          type="checkbox"
          checked={s.activityBasedTiming}
          onchange={(e) => put("activityBasedTiming", e.currentTarget.checked)}
        />
      </div>
      {#if s.activityBasedTiming}
        <div class="row indent">
          <label>{t("idleThresholdLabel")}</label>
          <select
            value={String(s.idleThresholdMinutes)}
            onchange={(e) =>
              put("idleThresholdMinutes", Number(e.currentTarget.value))}
          >
            {#each IDLE_THRESHOLDS as m}
              <option value={String(m)}>{t("minutes", { m })}</option>
            {/each}
          </select>
        </div>
        <p class="hint">{t("usageTimingHint")}</p>
      {/if}

      <div class="row">
        <label>{t("autoStartBreakLabel")}</label>
        <select
          value={s.autoStartBreak ? String(s.autoStartBreakDelaySeconds) : "0"}
          onchange={(e) => setCountdown(e.currentTarget.value)}
        >
          <option value="0">{t("noCountdown")}</option>
          {#each AUTO_DELAYS as sec}
            <option value={String(sec)}>{t("seconds", { s: sec })}</option>
          {/each}
        </select>
      </div>
      <p class="hint">{t("autoStartBreakHint")}</p>

      <div class="row">
        <label>{t("snoozeDurationLabel")}</label>
        <select
          value={isCustomSnooze ? "custom" : String(s.snoozeMinutes)}
          onchange={(e) =>
            e.currentTarget.value === "custom"
              ? null
              : put("snoozeMinutes", Number(e.currentTarget.value))}
        >
          {#each SNOOZE_QUICK as m}
            <option value={String(m)}>{t("minutes", { m })}</option>
          {/each}
          {#if isCustomSnooze}
            <option value="custom">{t("snoozeCustomPrefix")} · {s.snoozeMinutes} {t("snoozeCustomUnit")}</option>
          {/if}
        </select>
      </div>
      {#if isCustomSnooze}
        <div class="row indent">
          <label>{t("snoozeCustomPrefix")}</label>
          <input
            class="text-input"
            placeholder={t("snoozeCustomPrefix")}
            bind:value={customSnoozeText}
            onchange={commitCustomSnooze}
          />
        </div>
      {/if}
      <p class="hint">{t("snoozeCaption")}</p>
    </section>

    <!-- ============ 图片 ============ -->
    <section>
      <h2>{t("sectionWallpaper")}</h2>
      <div class="row top">
        <label></label>
        <div class="preview-wrap">
          {#if wallpaperPreview}
            <img class="preview" src={wallpaperPreview} alt="" />
          {:else}
            <div class="preview empty"></div>
          {/if}
          <button class="push" type="button" disabled={switching} onclick={switchWallpaper}>
            {switching ? t("switchWallpaperLoading") : t("switchWallpaper")}
          </button>
        </div>
      </div>
    </section>

    <!-- ============ 系统 ============ -->
    <section>
      <h2>{t("sectionSystem")}</h2>
      <div class="row">
        <label>{t("launchAtLoginLabel")}</label>
        <input
          type="checkbox"
          checked={s.launchAtLogin}
          onchange={(e) => put("launchAtLogin", e.currentTarget.checked)}
        />
      </div>
      <div class="row">
        <label>{t("soundLabel")}</label>
        <input
          type="checkbox"
          checked={s.soundEnabled}
          onchange={(e) => put("soundEnabled", e.currentTarget.checked)}
        />
      </div>
      <div class="row">
        <label>{t("overlayLabel")}</label>
        <input
          type="checkbox"
          checked={s.overlayOtherWindows}
          onchange={(e) => put("overlayOtherWindows", e.currentTarget.checked)}
        />
      </div>
    </section>

    <!-- ============ 提醒窗口 ============ -->
    <section>
      <h2>{t("sectionWindow")}</h2>
      <div class="row">
        <label>{t("windowOpacityLabel")}</label>
        <span class="slider-row">
          <input
            type="range"
            min="0.3"
            max="1"
            step="0.01"
            value={s.reminderWindowOpacity}
            oninput={(e) => put("reminderWindowOpacity", Number(e.currentTarget.value))}
          />
          <span class="value mono pct">
            {Math.round(s.reminderWindowOpacity * 100)}%
          </span>
        </span>
      </div>
      <p class="hint">{t("windowOpacityHint")}</p>
    </section>
  </div>

  <!-- ============ 更新弹窗（双通道） ============ -->
  {#if updateInfo}
    <div class="modal-mask">
      <div class="modal" role="dialog" aria-modal="true">
        <h3 class="modal-title">{t("updateAvailable", { v: updateInfo.version })}</h3>
        {#if updateInfo.notes.trim()}
          <pre class="modal-notes">{updateInfo.notes.trim()}</pre>
        {/if}

        {#if updatePhase === "downloading"}
          <p class="modal-status">{t("updateDownloading", { p: downloadPct })}</p>
          <div class="progress">
            <div class="progress-bar" style={`width: ${downloadPct}%`}></div>
          </div>
        {:else if updatePhase === "installing"}
          <p class="modal-status">{t("updateInstalling")}</p>
        {/if}

        {#if updatePhase === "idle"}
          <div class="modal-buttons">
            {#if updateInfo.inAppAvailable}
              <button class="primary" type="button" onclick={installNow}>
                {t("updateNow")}
              </button>
            {/if}
            <button type="button" onclick={openGithubRelease}>
              {t("updateFromGithub")}
            </button>
          </div>
          {#if !updateInfo.inAppAvailable}
            <p class="hint">{t("updateGithubOnlyHint")}</p>
          {/if}
          <div class="modal-secondary">
            <button class="link" type="button" onclick={skipThisVersion}>
              {t("skipUpdateVersion")}
            </button>
            <button class="link" type="button" onclick={() => (updateInfo = null)}>
              {t("updateLater")}
            </button>
          </div>
        {/if}
      </div>
    </div>
  {/if}
{/if}

<style>
  :global(html, body) {
    margin: 0;
    background: transparent;
  }
  .form {
    padding: 16px 18px 24px;
    font-family: -apple-system, system-ui, sans-serif;
    font-size: 13px;
    color: -apple-system-text, #1d1d1f;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  @media (prefers-color-scheme: dark) {
    .form { color: #f5f5f7; }
    section { background: rgba(60, 60, 67, 0.22) !important; }
    .hint { color: rgba(235, 235, 245, 0.6) !important; }
    .text-input, .num {
      background: rgba(40, 40, 45, 0.6) !important; color: inherit !important;
    }
  }
  section {
    background: rgba(127, 127, 129, 0.12);
    border-radius: 10px;
    padding: 8px 14px;
  }
  h2 {
    font-size: 12px;
    font-weight: 600;
    opacity: 0.55;
    margin: 4px 0 2px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 0;
    border-top: 1px solid rgba(128, 128, 130, 0.16);
  }
  .row:first-of-type { border-top: none; }
  .row.top { align-items: flex-start; }
  .row.indent { padding-left: 14px; }
  label { flex-shrink: 0; max-width: 58%; }
  .value { opacity: 0.65; }
  .mono { font-variant-numeric: tabular-nums; }
  .hint {
    font-size: 11.5px;
    line-height: 1.35;
    opacity: 0.6;
    margin: 2px 0 4px;
  }
  select, .text-input {
    min-width: 180px;
    font-size: 13px;
    padding: 3px 6px;
    border-radius: 6px;
    border: 1px solid rgba(128, 128, 130, 0.25);
    background: transparent;
    color: inherit;
  }
  .text-input { width: 90px; text-align: right; }
  .stepper {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }
  .stepper button, .push {
    appearance: none;
    border: 1px solid rgba(128, 128, 130, 0.35);
    background: rgba(127, 127, 129, 0.14);
    border-radius: 6px;
    padding: 2px 9px;
    font-size: 13px;
    cursor: pointer;
    color: inherit;
  }
  .stepper button:disabled { opacity: 0.35; cursor: default; }
  .preview-wrap {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
  }
  .preview {
    height: 110px;
    width: 190px;
    object-fit: cover;
    border-radius: 8px;
    outline: 1px solid rgba(128, 128, 130, 0.28);
    background: rgba(127, 127, 129, 0.1);
  }
  .slider-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    min-width: 210px;
  }
  .slider-row input[type="range"] { flex: 1; accent-color: #0a84ff; }
  .pct { min-width: 44px; text-align: right; opacity: 0.65; }
  .update-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }

  /* ---------- 更新弹窗 ---------- */
  .modal-mask {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 400px;
    max-width: calc(100% - 32px);
    max-height: calc(100% - 48px);
    overflow: auto;
    background: #f7f7f8;
    color: #1d1d1f;
    border-radius: 12px;
    padding: 18px 20px 14px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.28);
  }
  @media (prefers-color-scheme: dark) {
    .modal { background: #2c2c2e; color: #f5f5f7; }
  }
  .modal-title {
    margin: 0 0 10px;
    font-size: 15px;
    font-weight: 700;
  }
  .modal-notes {
    margin: 0 0 12px;
    font-size: 12px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 180px;
    overflow: auto;
    opacity: 0.85;
    font-family: inherit;
  }
  .modal-status {
    font-size: 13px;
    margin: 4px 0 8px;
  }
  .progress {
    height: 6px;
    border-radius: 3px;
    background: rgba(128, 128, 130, 0.25);
    overflow: hidden;
    margin-bottom: 12px;
  }
  .progress-bar {
    height: 100%;
    background: #0a84ff;
    border-radius: 3px;
    transition: width 0.2s ease-out;
  }
  .modal-buttons {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .modal-buttons button {
    appearance: none;
    border: 1px solid rgba(128, 128, 130, 0.35);
    background: rgba(127, 127, 129, 0.14);
    border-radius: 7px;
    padding: 6px 14px;
    font-size: 13px;
    cursor: pointer;
    color: inherit;
  }
  .modal-buttons button:hover { background: rgba(127, 127, 129, 0.24); }
  .modal-buttons button.primary {
    border-color: #0a84ff;
    background: #0a84ff;
    color: #fff;
    font-weight: 600;
  }
  .modal-buttons button.primary:hover { background: #3a9bff; }
  .modal-secondary {
    display: flex;
    justify-content: space-between;
    margin-top: 10px;
  }
  .link {
    appearance: none;
    border: none;
    background: none;
    color: #0a84ff;
    font-size: 12px;
    cursor: pointer;
    padding: 2px 4px;
  }
</style>
