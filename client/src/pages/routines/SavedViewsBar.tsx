import { useState } from "react";
import type { SavedView, ViewSnapshot } from "./savedViews";

export interface SavedViewsBarProps {
  views: SavedView[];
  onApply: (snapshot: ViewSnapshot) => void;
  onSave: (name: string) => void;
  onDelete: (name: string) => void;
}

/** Dropdown to apply a saved view, plus inline controls to save/delete named presets. */
export function SavedViewsBar({ views, onApply, onSave, onDelete }: SavedViewsBarProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  // ponytail: tracks only which view was last picked/saved, for the DELETE button and the
  // select's displayed value — doesn't clear when the operator hand-edits a filter afterwards.
  // Harmless (re-picking/re-saving fixes it); not worth watching every filter field for it.
  const [picked, setPicked] = useState("");

  const onPick = (name: string) => {
    const view = views.find((v) => v.name === name);
    if (view) onApply(view.snapshot);
    setPicked(name);
  };

  const onConfirmSave = () => {
    const name = draft.trim();
    if (name === "") return;
    onSave(name);
    setPicked(name);
    setEditing(false);
  };

  const onDeleteClick = () => {
    if (picked === "") return;
    onDelete(picked);
    setPicked("");
  };

  return (
    <div className="filter-bar saved-views-bar">
      <div className="filter-field">
        <span className="filter-label">VIEWS</span>
        <select
          className="filter-select"
          aria-label="Saved views"
          value={picked}
          onChange={(e) => onPick(e.target.value)}
        >
          <option value="">— select —</option>
          {views.map((v) => (
            <option key={v.name} value={v.name}>
              {v.name}
            </option>
          ))}
        </select>
        {editing ? (
          <>
            <input
              type="text"
              className="filter-input"
              placeholder="View name…"
              aria-label="New view name"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
            />
            <button type="button" className="btn btn-primary btn-sm" disabled={draft.trim() === ""} onClick={onConfirmSave}>
              SAVE
            </button>
            <button type="button" className="btn btn-ghost btn-sm" onClick={() => setEditing(false)}>
              CANCEL
            </button>
          </>
        ) : (
          <>
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              title="Save current filters, sort, and grouping as a named view"
              onClick={() => {
                setEditing(true);
                setDraft("");
              }}
            >
              ☆ SAVE VIEW
            </button>
            {picked !== "" && (
              <button type="button" className="btn btn-ghost btn-sm" title="Delete this saved view" onClick={onDeleteClick}>
                DELETE
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}
