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

export function MoveRoutineDialog({ routine, saving, onCancel, onConfirm }: MoveRoutineDialogProps) {
  const [folder, setFolder] = useState(splitFolder(routine.folder));
  const [slug, setSlug] = useState(splitSlug(routine.slug));
  const folderValue = folder.trim();
  const slugValue = slug.trim();
  const relPath = useMemo(
    () => (folderValue === "" ? slugValue : `${folderValue}/${slugValue}`),
    [folderValue, slugValue],
  );
  const unchanged = folderValue === splitFolder(routine.folder) && slugValue === splitSlug(routine.slug);
  const invalid = slugValue === "" || slugValue.includes("/") || slugValue.includes("\\");

  return (
    <div className="overlay" onClick={onCancel}>
      <div className="dialog move-dialog" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">MOVE ROUTINE</div>
        <div className="dialog-msg">
          Move <strong>{routine.title}</strong> to a filesystem folder.
        </div>
        <div className="form-group">
          <label className="form-label" htmlFor="move-folder">
            FOLDER
          </label>
          <input
            id="move-folder"
            className="form-input"
            placeholder="maintenance"
            autoComplete="off"
            spellCheck={false}
            value={folder}
            onChange={(e) => setFolder(e.target.value)}
          />
          <div className="form-hint">Blank keeps the routine at the root. Use / for nested folders.</div>
        </div>
        <div className="form-group">
          <label className="form-label" htmlFor="move-slug">
            SLUG*
          </label>
          <input
            id="move-slug"
            className="form-input"
            placeholder="daily-maintenance-digest"
            autoComplete="off"
            spellCheck={false}
            value={slug}
            onChange={(e) => setSlug(e.target.value)}
          />
          <div className="form-hint">One path segment only. Title remains display-only.</div>
        </div>
        <div className="move-preview">routines/{relPath || "…"}/routine.toml</div>
        <div className="dialog-actions">
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={saving || invalid || unchanged}
            onClick={() => onConfirm(folderValue === "" ? undefined : folderValue, slugValue)}
          >
            {saving ? "Moving…" : "Move"}
          </button>
        </div>
      </div>
    </div>
  );
}
