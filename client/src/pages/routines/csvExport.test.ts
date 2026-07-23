import { describe, expect, it, vi } from "vitest";
import type { RoutineResponse } from "../../api/hooks";
import { downloadCsv, routinesToCsv } from "./csvExport";

function routine(overrides: Partial<RoutineResponse> = {}): RoutineResponse {
  return {
    id: "r1",
    title: "Nightly build",
    schedule: "0 2 * * *",
    agent: "claude",
    enabled: true,
    source: "local",
    created_at: 0,
    updated_at: 0,
    agent_registered: true,
    agent_command_available: true,
    agent_setup_available: true,
    file_path: "/tmp/r1/routine.toml",
    flag_count: 0,
    is_running: false,
    env_keys: [],
    ...overrides,
  } as RoutineResponse;
}

describe("routinesToCsv", () => {
  it("emits just the header for an empty list", () => {
    expect(routinesToCsv([])).toBe(
      "id,title,enabled,status,schedule,schedule_description,timezone,agent,model,machines,tags,flag_count,next_run_at,last_scheduled_trigger_at,last_manual_trigger_at\r\n",
    );
  });

  it("renders a fully-populated routine, joining arrays with ';' and epochs as ISO", () => {
    const csv = routinesToCsv([
      routine({
        machines: ["mac-1", "mac-2"],
        tags: ["nightly", "triage"],
        model: "claude-sonnet-4-6",
        schedule_description: "daily at 02:00",
        timezone: "UTC",
        next_run_at: 86400,
        last_scheduled_trigger_at: 0,
        flag_count: 3,
      }),
    ]);
    const rows = csv.trim().split("\r\n");
    expect(rows).toHaveLength(2);
    expect(rows[1]).toBe(
      "r1,Nightly build,true,enabled,0 2 * * *,daily at 02:00,UTC,claude,claude-sonnet-4-6,mac-1;mac-2,nightly;triage,3,1970-01-02T00:00:00.000Z,1970-01-01T00:00:00.000Z,",
    );
  });

  it("maps missing optional/array fields to empty strings", () => {
    const csv = routinesToCsv([routine()]);
    const [, row] = csv.trim().split("\r\n");
    // model,machines,tags,next_run_at,last_scheduled_trigger_at,last_manual_trigger_at all blank
    expect(row).toBe("r1,Nightly build,true,enabled,0 2 * * *,,,claude,,,,0,,,");
  });

  it("marks a disabled routine as disabled regardless of is_running", () => {
    const csv = routinesToCsv([routine({ enabled: false, is_running: true })]);
    expect(csv).toContain(",disabled,");
  });

  it("marks an enabled, currently-firing routine as running", () => {
    const csv = routinesToCsv([routine({ is_running: true })]);
    expect(csv).toContain(",running,");
  });

  it("quotes fields containing a comma, quote, or newline and doubles embedded quotes", () => {
    const csv = routinesToCsv([routine({ title: 'Say "hi", then\nnewline' })]);
    const [, row] = csv.trim().split("\r\n");
    expect(row).toContain('"Say ""hi"", then\nnewline"');
  });
});

describe("downloadCsv", () => {
  it("creates an object URL, clicks a download anchor, then revokes the URL", () => {
    const createObjectURL = vi.fn(() => "blob:mock-url");
    const revokeObjectURL = vi.fn();
    vi.stubGlobal("URL", { ...URL, createObjectURL, revokeObjectURL });
    const click = vi.fn();
    const anchor = { click, href: "", download: "" } as unknown as HTMLAnchorElement;
    const createElementSpy = vi.spyOn(document, "createElement").mockReturnValue(anchor);

    downloadCsv("routines.csv", "id,title\r\n");

    expect(createObjectURL).toHaveBeenCalledOnce();
    expect(anchor.download).toBe("routines.csv");
    expect(click).toHaveBeenCalledOnce();
    expect(revokeObjectURL).toHaveBeenCalledWith("blob:mock-url");

    createElementSpy.mockRestore();
    vi.unstubAllGlobals();
  });
});
