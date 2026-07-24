import type { RoutineResponse } from "../../api/hooks";

const COLUMNS = [
  "id",
  "title",
  "enabled",
  "status",
  "schedule",
  "schedule_description",
  "timezone",
  "agent",
  "model",
  "machines",
  "tags",
  "flag_count",
  "next_run_at",
  "last_scheduled_trigger_at",
  "last_manual_trigger_at",
] as const;

function toIso(epochSecs: number | null | undefined): string {
  if (epochSecs == null) return "";
  const d = new Date(epochSecs * 1000);
  return Number.isNaN(d.getTime()) ? "" : d.toISOString();
}

function statusOf(r: RoutineResponse): string {
  if (!r.enabled) return "disabled";
  if (r.is_running) return "running";
  return "enabled";
}

/** Quote a CSV field per RFC 4180 whenever it contains a comma, quote, or newline. */
function csvField(value: string): string {
  return /[",\n\r]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;
}

function csvRow(values: string[]): string {
  return values.map(csvField).join(",");
}

/**
 * Serializes routines to CSV using the exact rows passed in — callers pass the
 * already filtered/sorted/grouped `visible` list so the export always matches
 * what the operator currently sees on screen, not the full unfiltered fleet.
 */
export function routinesToCsv(routines: RoutineResponse[]): string {
  const lines = [csvRow([...COLUMNS])];
  for (const r of routines) {
    lines.push(
      csvRow([
        r.id,
        r.title,
        String(r.enabled),
        statusOf(r),
        r.schedule,
        r.schedule_description ?? "",
        r.timezone ?? "",
        r.agent,
        r.model ?? "",
        (r.machines ?? []).join(";"),
        (r.tags ?? []).join(";"),
        String(r.flag_count),
        toIso(r.next_run_at),
        toIso(r.last_scheduled_trigger_at),
        toIso(r.last_manual_trigger_at),
      ]),
    );
  }
  return lines.join("\r\n") + "\r\n";
}

/** Triggers a browser download of `content` as a file named `filename`. */
export function downloadCsv(filename: string, content: string): void {
  const blob = new Blob([content], { type: "text/csv;charset=utf-8;" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  a.click();
  URL.revokeObjectURL(url);
}
