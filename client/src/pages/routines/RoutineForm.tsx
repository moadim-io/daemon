import { useEffect } from "react";
import { useForm, useWatch } from "react-hook-form";
import { z } from "zod";
import { useAgents } from "../../api/hooks";
import type { components } from "../../api/schema.gen";
import { describeCronLive } from "../../lib/cronUtils";
import { MachinesPicker } from "./MachinesPicker";
import { parseTtl, reposToText, tagsToText, textToRepos, textToTags } from "./routineDraft";
import { TTL_PRESETS } from "./ttl";

type Repository = components["schemas"]["Repository"];

export interface RoutineDraft {
  schedule: string;
  title: string;
  agent: string;
  model: string | null;
  prompt: string;
  goal: string | null;
  repositories: Repository[];
  machines: string[];
  enabled: boolean;
  ttl_secs: number | null;
  tags: string[];
}

interface FormValues {
  title: string;
  schedule: string;
  agent: string;
  model: string;
  prompt: string;
  goal: string;
  reposRaw: string;
  machines: string[];
  tagsRaw: string;
  ttlRaw: string;
  enabled: boolean;
}

const nonBlank = z.string().trim().min(1);

const FALLBACK_AGENTS = ["claude", "codex"];

const CRON_PRESETS: [string, string][] = [
  ["@daily", "@daily"],
  ["@hourly", "@hourly"],
  ["@weekly", "@weekly"],
  ["@monthly", "@monthly"],
  ["0 0 9 * * 1-5 *", "weekdays 9am"],
  ["0 0 * * * * *", "every hour"],
];

function draftToValues(draft?: Partial<RoutineDraft>): FormValues {
  return {
    title: draft?.title ?? "",
    schedule: draft?.schedule ?? "",
    agent: draft?.agent ?? "claude",
    model: draft?.model ?? "",
    prompt: draft?.prompt ?? "",
    goal: draft?.goal ?? "",
    reposRaw: reposToText(draft?.repositories ?? []),
    machines: draft?.machines ?? [],
    tagsRaw: tagsToText(draft?.tags ?? []),
    ttlRaw: draft?.ttl_secs != null ? String(draft.ttl_secs) : "",
    enabled: draft?.enabled ?? true,
  };
}

export interface RoutineFormProps {
  /** `undefined` = blank create form. */
  initial?: Partial<RoutineDraft>;
  mode: "create" | "edit" | "clone";
  saving: boolean;
  onCancel: () => void;
  onSave: (draft: RoutineDraft) => void;
}

export function RoutineForm({ initial, mode, saving, onCancel, onSave }: RoutineFormProps) {
  const agentsQuery = useAgents();
  const agents = agentsQuery.data && agentsQuery.data.length > 0 ? agentsQuery.data : FALLBACK_AGENTS;

  const { register, control, setValue, handleSubmit, reset } = useForm<FormValues>({
    defaultValues: draftToValues(initial),
  });

  // Re-seed the form whenever the routine being edited/cloned changes identity.
  useEffect(() => {
    reset(draftToValues(initial));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initial]);

  const title = useWatch({ control, name: "title" });
  const schedule = useWatch({ control, name: "schedule" });
  const agent = useWatch({ control, name: "agent" });
  const prompt = useWatch({ control, name: "prompt" });
  const machines = useWatch({ control, name: "machines" });
  const canSave =
    nonBlank.safeParse(title).success &&
    nonBlank.safeParse(schedule).success &&
    nonBlank.safeParse(agent).success &&
    nonBlank.safeParse(prompt).success;

  const [cronOk, cronText] = describeCronLive(schedule);
  const previewClass =
    schedule.trim() === "" ? "cron-preview" : cronOk ? "cron-preview ok" : "cron-preview bad";

  const submit = handleSubmit((v) => {
    onSave({
      schedule: v.schedule,
      title: v.title,
      agent: v.agent,
      model: v.model.trim() === "" ? null : v.model,
      prompt: v.prompt,
      goal: v.goal.trim() === "" ? null : v.goal,
      repositories: textToRepos(v.reposRaw),
      machines: v.machines,
      enabled: v.enabled,
      ttl_secs: parseTtl(v.ttlRaw),
      tags: textToTags(v.tagsRaw),
    });
  });

  const isEdit = mode === "edit";
  const submitLabel = saving ? "…" : isEdit ? "SAVE CHANGES" : "CREATE ROUTINE";

  const body = (
    <>
      <div className="form-group">
        <label className="form-label">TITLE*</label>
        <input
          className="form-input"
          placeholder="ops/nightly triage"
          autoComplete="off"
          spellCheck={false}
          {...register("title")}
        />
        <div className="form-hint">Use / to nest folders and subfolders.</div>
      </div>

      <div className="form-group">
        <label className="form-label">SCHEDULE*</label>
        <input
          className="form-input"
          placeholder="sec min hour dom month dow year"
          {...register("schedule")}
        />
        <div className="cron-presets">
          {CRON_PRESETS.map(([value, label]) => (
            <button
              type="button"
              key={value}
              className="preset-btn"
              onClick={() => setValue("schedule", value)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className={previewClass}>{cronText}</div>
      </div>

      <div className="form-group">
        <label className="form-label">AGENT*</label>
        <select className="form-select" {...register("agent")}>
          {agents.map((a) => (
            <option key={a} value={a}>
              {a}
            </option>
          ))}
        </select>
      </div>

      <div className="form-group">
        <label className="form-label">
          MODEL <span style={{ color: "var(--text-faint)" }}>(optional; blank = agent default)</span>
        </label>
        <input className="form-input" placeholder="claude-sonnet-4-6" {...register("model")} />
      </div>

      <div className="form-group">
        <label className="form-label">PROMPT*</label>
        <textarea
          className="form-textarea"
          rows={6}
          placeholder="Review open PRs and summarize…"
          {...register("prompt")}
        />
      </div>

      <div className="form-group">
        <label className="form-label">
          GOAL <span style={{ color: "var(--text-faint)" }}>(optional; ≤5 lines — the why)</span>
        </label>
        <textarea
          className="form-textarea"
          rows={3}
          placeholder="Keep the PR backlog under control…"
          {...register("goal")}
        />
      </div>

      <div className="form-group">
        <label className="form-label">
          REPOSITORIES <span style={{ color: "var(--text-faint)" }}>(one url [branch] per line)</span>
        </label>
        <textarea
          className="form-textarea"
          rows={3}
          placeholder="https://github.com/org/repo main"
          {...register("reposRaw")}
        />
      </div>

      <MachinesPicker value={machines} onChange={(m) => setValue("machines", m)} />

      <div className="form-group">
        <label className="form-label">
          TAGS <span style={{ color: "var(--text-faint)" }}>(comma-separated)</span>
        </label>
        <input className="form-input" placeholder="triage, nightly" {...register("tagsRaw")} />
      </div>

      <div className="form-group">
        <label className="form-label">
          WORKBENCH TTL <span style={{ color: "var(--text-faint)" }}>(seconds; blank = server default)</span>
        </label>
        <input className="form-input" type="number" min={0} placeholder="604800" {...register("ttlRaw")} />
        <div className="ttl-presets">
          {TTL_PRESETS.map(([secs, label]) => (
            <button type="button" key={secs} className="preset-btn" onClick={() => setValue("ttlRaw", secs)}>
              {label}
            </button>
          ))}
        </div>
      </div>

      <div className="toggle-row">
        <span>ENABLED</span>
        <label className="toggle">
          <input type="checkbox" {...register("enabled")} />
          <div className="toggle-track" />
        </label>
      </div>
    </>
  );

  const footer = (
    <div className="modal-ft">
      <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
        CANCEL
      </button>
      <button type="button" className="btn btn-primary btn-sm" disabled={saving || !canSave} onClick={submit}>
        {submitLabel}
      </button>
    </div>
  );

  if (isEdit) {
    return (
      <div className="overlay open">
        <div className="dialog modal">
          <div className="modal-hd">
            <div className="modal-title">EDIT ROUTINE</div>
            <button type="button" className="modal-x" title="Close" aria-label="Close" onClick={onCancel}>
              ✕
            </button>
          </div>
          <div className="modal-body">{body}</div>
          {footer}
        </div>
      </div>
    );
  }

  return (
    <main className="create-page">
      <div className="page-hd">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          ← BACK
        </button>
        <div className="page-title">{mode === "clone" ? "NEW ROUTINE (FROM CLONE)" : "NEW ROUTINE"}</div>
      </div>
      <div className="page-card card">
        <div className="modal-body">{body}</div>
        {footer}
      </div>
    </main>
  );
}
