import { useMemo, useState } from "react";
import type { RoutineResponse } from "../../api/hooks";

export interface MoveRoutineDialogProps {
  routine: RoutineResponse;
  saving: boolean;
  onCancel: () => void;
  onConfirm: (folder: string | undefined, slug: string) => void;
}

function splitFolder(value: string | null | undefined): string {
  return value?.trim() ?? "";
}

function splitSlug(value: string | undefined): string {
  return value?.trim() ?? "";
}

function locationPath(folder: string, slug: string): string {
  return folder === "" ? slug : `${folder}/${slug}`;
}

export function MoveRoutineDialog({ routine, saving, onCancel, onConfirm }: MoveRoutineDialogProps) {
  const [folder, setFolder] = useState(splitFolder(routine.folder));
  const [slug, setSlug] = useState(splitSlug(routine.slug));
  const folderValue = folder.trim();
  const slugValue = slug.trim();
  const relPath = useMemo(() => locationPath(folderValue, slugValue), [folderValue, slugValue]);
  const currentPath = useMemo(
    () => locationPath(splitFolder(routine.folder), splitSlug(routine.slug)),
    [routine.folder, routine.slug],
  );
  const unchanged = folderValue === splitFolder(routine.folder) && slugValue === splitSlug(routine.slug);
  const invalid = slugValue === "" || slugValue.includes("/") || slugValue.includes("\\");

  return (
    <div className="overlay" onClick={onCancel}>
      <div
        className="dialog move-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="move-dialog-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="move-dialog-hd">
          <div>
            <div id="move-dialog-title" className="dialog-title move-dialog-title">FOLDER MANAGEMENT</div>
            <div className="move-dialog-sub">Move routine files without changing the display title.</div>
          </div>
          <div className="move-dialog-badge">FILES</div>
        </div>

        <div className="move-current-card" aria-label={`Current path ${currentPath}`}>
          <div className="move-current-label">CURRENT LOCATION</div>
          <div className="move-current-row">
            <span className="routine-fs-icon" aria-hidden="true">📄</span>
            <div className="move-current-copy">
              <strong>{routine.title}</strong>
              <span>{currentPath || "root"}</span>
            </div>
          </div>
        </div>

        <div className="move-path-grid">
          <div className="form-group move-form-group">
            <label className="form-label" htmlFor="move-folder">
              FOLDER
            </label>
            <input
              id="move-folder"
              className="form-input move-input"
              placeholder="maintenance"
              autoComplete="off"
              spellCheck={false}
              value={folder}
              onChange={(e) => setFolder(e.target.value)}
            />
            <div className="form-hint">Blank keeps the routine at root. Use / for nested folders.</div>
          </div>
          <div className="form-group move-form-group">
            <label className="form-label" htmlFor="move-slug">
              SLUG*
            </label>
            <input
              id="move-slug"
              className="form-input move-input"
              placeholder="daily-maintenance-digest"
              autoComplete="off"
              spellCheck={false}
              value={slug}
              onChange={(e) => setSlug(e.target.value)}
            />
            <div className="form-hint">One path segment only. Title remains display-only.</div>
          </div>
        </div>

        <div className="move-preview" aria-label={`Destination path routines/${relPath || "…"}/routine.toml`}>
          <div className="move-preview-label">DESTINATION PREVIEW</div>
          <div className="move-preview-path">routines/{relPath || "…"}/routine.toml</div>
        </div>

        <div className="dialog-actions move-dialog-actions">
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            CANCEL
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving || invalid || unchanged}
            onClick={() => onConfirm(folderValue === "" ? undefined : folderValue, slugValue)}
          >
            {saving ? "MOVING…" : "MOVE"}
          </button>
        </div>
      </div>
    </div>
  );
}
