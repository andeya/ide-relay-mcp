/**
 * Minimal i18n: `en` / `zh` message maps and `{key}` interpolation for Vue UI.
 */
import { ref, type Ref } from "vue";
import en from "./locales/en";
import zh from "./locales/zh";

export type Locale = "en" | "zh";
type Messages = typeof en;

const messages: Record<Locale, Messages> = { en, zh };

export const locale: Ref<Locale> = ref("en");

function interpolate(template: string, vars: Record<string, string>): string {
  return template.replace(/\{(\w+)\}/g, (_, k) => vars[k] ?? `{${k}}`);
}

export function t(key: keyof Messages, vars?: Record<string, string>): string {
  let s = messages[locale.value][key] ?? messages.en[key] ?? String(key);
  if (vars) {
    s = interpolate(s, vars);
  }
  return s;
}
