import { NAV_CHORDS } from "../lib/keyNav";

export interface ShortcutsHelpProps {
  onClose: () => void;
}

/** `?` toggles this cheat sheet — the discoverability half of the shell's keyboard-first navigation. */
export function ShortcutsHelp({ onClose }: ShortcutsHelpProps) {
  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="dialog shortcuts-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="shortcuts-dialog-title"
        onClick={(e) => e.stopPropagation()}
      >
        <div id="shortcuts-dialog-title" className="dialog-title">
          ⌨ Keyboard shortcuts
        </div>
        <ul className="shortcuts-list">
          <li>
            <span className="cmdk-key">⌘K</span>
            <span className="cmdk-key">Ctrl K</span>
            <span>Open command palette</span>
          </li>
          <li>
            <span className="cmdk-key">?</span>
            <span>Toggle this help</span>
          </li>
          <li>
            <span className="cmdk-key">Esc</span>
            <span>Close any open dialog</span>
          </li>
          {NAV_CHORDS.map((chord) => (
            <li key={chord.key}>
              <span className="cmdk-key">G</span>
              <span className="cmdk-key">{chord.key.toUpperCase()}</span>
              <span>Go to {chord.label}</span>
            </li>
          ))}
        </ul>
        <div className="dialog-actions">
          <button className="btn btn-ghost" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
