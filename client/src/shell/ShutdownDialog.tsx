export interface ShutdownDialogProps {
  onCancel: () => void;
  onConfirm: () => void;
}

export function ShutdownDialog({ onCancel, onConfirm }: ShutdownDialogProps) {
  return (
    <div className="overlay" onClick={onCancel}>
      <div
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shutdown-dialog-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div id="shutdown-dialog-title" className="dialog-title">
          ⏻ Stop server
        </div>
        <div className="dialog-msg">
          Stop the moadim server? Scheduled jobs and routines will not run until it is started
          again.
        </div>
        <div className="dialog-actions">
          <button className="btn btn-ghost" onClick={onCancel}>
            Cancel
          </button>
          <button className="btn btn-danger" onClick={onConfirm}>
            Stop server
          </button>
        </div>
      </div>
    </div>
  );
}
