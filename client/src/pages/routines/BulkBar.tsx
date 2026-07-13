export interface BulkBarProps {
  count: number;
  onEnable: () => void;
  onDisable: () => void;
  onDelete: () => void;
  onClear: () => void;
}

/** Action bar shown above the table when one or more routines are selected. */
export function BulkBar({ count, onEnable, onDisable, onDelete, onClear }: BulkBarProps) {
  if (count === 0) return null;
  return (
    <div className="bulk-bar">
      <span className="bulk-count">{count} SELECTED</span>
      <div className="bulk-acts">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onEnable}>
          ENABLE
        </button>
        <button type="button" className="btn btn-ghost btn-sm" onClick={onDisable}>
          DISABLE
        </button>
        <button type="button" className="btn btn-danger btn-sm" onClick={onDelete}>
          DELETE
        </button>
        <button type="button" className="btn btn-ghost btn-sm" onClick={onClear}>
          CLEAR
        </button>
      </div>
    </div>
  );
}

export interface BulkDeleteDialogProps {
  count: number;
  onCancel: () => void;
  onConfirm: () => void;
}

export function BulkDeleteDialog({ count, onCancel, onConfirm }: BulkDeleteDialogProps) {
  return (
    <div className="overlay" onClick={onCancel}>
      <div className="dialog" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">⚠ DELETE ROUTINES</div>
        <div className="dialog-msg">
          Delete {count} selected routine{count === 1 ? "" : "s"}? This cannot be undone.
        </div>
        <div className="dialog-actions">
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn btn-danger" onClick={onConfirm}>
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}

export interface ConfirmDeleteDialogProps {
  title: string;
  onCancel: () => void;
  onConfirm: () => void;
}

export function ConfirmDeleteDialog({ title, onCancel, onConfirm }: ConfirmDeleteDialogProps) {
  return (
    <div className="overlay" onClick={onCancel}>
      <div className="dialog" role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-title">⚠ DELETE ROUTINE</div>
        <div className="dialog-msg">Delete the routine &quot;{title}&quot;? This cannot be undone.</div>
        <div className="dialog-actions">
          <button type="button" className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn btn-danger" onClick={onConfirm}>
            Delete
          </button>
        </div>
      </div>
    </div>
  );
}
