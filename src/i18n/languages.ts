/**
 * Language metadata for the locales actually shipped in the UI.
 *
 * Other locales (zh, zh-TW, es, de, ja, ko, vi, pl, it, ru, uk, pt, cs,
 * tr, ar) have partial translation files under `src/i18n/locales/` that
 * are kept for future reactivation but are 17% incomplete vs en/fr —
 * shipping them would mean half-French strings bleeding through the
 * marquee Voice Commands feature. We expose only the locales we can
 * stand behind today.
 *
 * To reactivate a locale:
 * 1. Fully sync `src/i18n/locales/{code}/translation.json` with en
 *    (use bun scripts/check-translations.ts to find gaps).
 * 2. Add the brace entry for `{code}` to the glob in `index.ts`.
 * 3. Add the metadata entry below.
 */
export const LANGUAGE_METADATA: Record<
  string,
  {
    name: string;
    nativeName: string;
    priority?: number;
    direction?: "ltr" | "rtl";
  }
> = {
  fr: { name: "French", nativeName: "Français", priority: 1 },
  en: { name: "English", nativeName: "English", priority: 2 },
};
