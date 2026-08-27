/// 与 Rust l10n.rs 单一事实来源同步的响应式字典。
/// 启动时 invoke get_strings 拉取全量，语言切换时由 strings-changed 推送更新。
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export const L10n = $state<{ lang: string; strings: Record<string, string> }>({
  lang: "english",
  strings: {},
});

export async function initI18n(): Promise<void> {
  const payload = await invoke<{
    lang: string;
    strings: Record<string, string>;
  }>("get_strings");
  setDict(payload);
  await listen<{ lang: string; strings: Record<string, string> }>(
    "strings-changed",
    (e) => setDict(e.payload),
  );
}

function setDict(p: { lang: string; strings: Record<string, string> }) {
  L10n.lang = p.lang;
  L10n.strings = p.strings;
}

/** 取词 + {name} 占位符替换，与 Rust tf() 同语义。 */
export function t(
  key: string,
  params?: Record<string, string | number>,
): string {
  let s = L10n.strings[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.replaceAll(`{${k}}`, String(v));
    }
  }
  return s;
}
