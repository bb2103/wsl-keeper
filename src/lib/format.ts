import type { LocaleId, TFunction } from "../i18n";
import { localeTag } from "../i18n";

export function formatBytes(bytes: number, t: TFunction): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return t("format.unknownSize");
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let i = 0;
  while (size >= 1024 && i < units.length - 1) {
    size /= 1024;
    i += 1;
  }
  return `${size.toFixed(size >= 10 || i === 0 ? 0 : 1)} ${units[i]}`;
}

export function formatDuration(fromIso: string | null, t: TFunction): string {
  if (!fromIso) return t("format.notYet");
  const from = new Date(fromIso).getTime();
  if (Number.isNaN(from)) return t("format.notYet");
  const minutes = Math.max(0, Math.floor((Date.now() - from) / 60000));
  const days = Math.floor(minutes / 1440);
  const hours = Math.floor((minutes % 1440) / 60);
  const mins = minutes % 60;
  if (days > 0) return t("format.daysHours", { days, hours });
  if (hours > 0) return t("format.hoursMins", { hours, mins });
  if (mins > 0) return t("format.mins", { mins });
  return t("format.justNow");
}

export function formatClock(iso: string | null, locale: LocaleId): string {
  if (!iso) return "";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return date.toLocaleString(localeTag(locale), {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
