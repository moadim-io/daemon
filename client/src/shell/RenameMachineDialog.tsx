import { useState } from "react";

export interface RenameMachineDialogProps {
  current: string;
  onCancel: () => void;
  /** Fires the API call; resolves/rejects so the dialog can reset its busy state. */
  onConfirm: (name: string) => Promise<void>;
}

export function RenameMachineDialog({ current, onCancel, onConfirm }: RenameMachineDialogProps) {
  const [draft, setDraft] = useState(current);
  const [busy, setBusy] = useState(false);

  const trimmed = draft.trim();

  const save = async () => {
    if (trimmed === "") return;
    setBusy(true);
    try {
      await onConfirm(trimmed);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="overlay" onClick={onCancel}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="rename-machine-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div id="rename-machine-title" className="dialog-title">
          Rename machine
        </div>
        <div className="dialog-msg">
          <label className="form-label" htmlFor="rename-machine-input">
            Machine name
          </label>
          <input
            id="rename-machine-input"
            className="form-input"
            type="text"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
          />
        </div>
        <div className="dialog-actions">
          <button className="btn btn-ghost" onClick={onCancel} disabled={busy}>
            Cancel
          </button>
          <button
            className="btn btn-primary"
            onClick={() => void save()}
            disabled={busy || trimmed === ""}
          >
            {busy ? "Saving…" : "Rename"}
          </button>
        </div>
      </div>
    </div>
  );
}
