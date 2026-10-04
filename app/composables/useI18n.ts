import { computed, readonly, ref } from "vue";
import pt from "~/i18n/pt.json";
import en from "~/i18n/en.json";
import es from "~/i18n/es.json";
import legacyKeys from "~/i18n/legacy-keys.json";

export type Locale = "pt" | "en" | "es";
export const defaultLocale: Locale = "en";
export const languages = [
  { value: "en", label: "English", tag: "en-US" },
  { value: "pt", label: "Português", tag: "pt-BR" },
  { value: "es", label: "Español", tag: "es-ES" },
] as const;
export const catalogs: Record<Locale, Record<string, string>> = { pt, en, es };
export const localeStorageKey = "proton-drive.locale";
export const messagePrefix = "\u001eproton-i18n:";
export type MessageValue =
  string | number | null | undefined | { message: string };

export function isLocale(value: unknown): value is Locale {
  return value === "pt" || value === "en" || value === "es";
}
export function savedLocale(): Locale {
  try {
    const value = localStorage.getItem(localeStorageKey);
    if (isLocale(value)) return value;
  } catch {
    /* Storage can be disabled in browser previews. */
  }
  return defaultLocale;
}
const selected = ref<Locale>(savedLocale());
const locale = computed(
  () => languages.find((item) => item.value === selected.value)!.tag,
);

export function setLocale(value: Locale) {
  const next = isLocale(value) ? value : defaultLocale;
  selected.value = next;
  try {
    localStorage.setItem(localeStorageKey, next);
  } catch {
    /* Native settings remain authoritative. */
  }
  if (typeof document !== "undefined")
    document.documentElement.lang = locale.value;
}

// Source messages use English keys. Legacy aliases keep saved activity readable.
// Parameters are data, never lookup keys: user filenames must remain unchanged.
export function encodeMessage(key: string, values: MessageValue[] = []) {
  return messagePrefix + JSON.stringify({ key, values });
}
export function useI18n() {
  const number = (value: number, options?: Intl.NumberFormatOptions) =>
    new Intl.NumberFormat(locale.value, options).format(value);
  function translate(
    key: string,
    values: MessageValue[] = [],
    depth = 0,
  ): string {
    if (Object.hasOwn(legacyKeys, key))
      key = legacyKeys[key as keyof typeof legacyKeys];
    const lookup = (dictionary: Record<string, string>) => {
      const value = Object.hasOwn(dictionary, key)
        ? dictionary[key]
        : undefined;
      return value?.trim() ? value : undefined;
    };
    const pattern =
      lookup(catalogs[selected.value]) ??
      lookup(catalogs[defaultLocale]) ??
      key;
    return pattern.replace(/\{(\d+)\}/g, (placeholder, index) => {
      const value = values[Number(index)];
      if (value === undefined || value === null)
        return value === null ? "" : placeholder;
      if (typeof value === "object") return resolve(value.message, depth + 1);
      return typeof value === "number" ? number(value) : value;
    });
  }
  function resolve(value: string, depth = 0): string {
    if (depth > 8) return value;
    if (value.startsWith("Error: " + messagePrefix)) value = value.slice(7);
    if (value.startsWith(messagePrefix)) {
      try {
        const parsed = JSON.parse(value.slice(messagePrefix.length));
        if (typeof parsed.key === "string" && Array.isArray(parsed.values))
          return translate(parsed.key, parsed.values, depth);
      } catch {
        /* Unknown external diagnostics remain readable. */
      }
    }
    // Also translates static messages saved by older app versions.
    return translate(value, [], depth);
  }
  const t = (key: string, values: MessageValue[] = []) =>
    translate(key, values);
  const plural = (one: string, other: string, count: number) =>
    t(
      count !== 0 && new Intl.PluralRules(locale.value).select(count) === "one"
        ? one
        : other,
      [count],
    );
  return {
    t,
    message: resolve,
    number,
    plural,
    locale,
    language: readonly(selected),
    setLocale,
    languages,
  };
}
