/**
 * Minimal i18n: `en` / `zh` message maps and `{key}` interpolation for Vue UI.
 */
import { ref, watch, type Ref } from "vue";
import en from "./locales/en";
import zh from "./locales/zh";

export type Locale = "en" | "zh";
type Messages = typeof en;

const messages: Record<Locale, Messages> = { en, zh };

const LANG_MAP: Record<Locale, string> = { en: "en", zh: "zh-CN" };

export const locale: Ref<Locale> = ref("en");

watch(locale, (l) => {
  document.documentElement.lang = LANG_MAP[l] || "en";
});

/** Resolve catalog key even if `locale` was ever set to a non-Locale string at runtime. */
function activeLocale(): Locale {
  return locale.value === "zh" ? "zh" : "en";
}

function interpolate(template: string, vars: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? `{${k}}`);
}

export function t(key: keyof Messages, vars?: Record<string, string>): string {
  let s = messages[activeLocale()][key] ?? messages.en[key] ?? String(key);
  if (vars) {
    s = interpolate(s, vars);
  }
  return s;
}
