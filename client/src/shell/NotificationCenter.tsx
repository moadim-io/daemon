import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { reltime } from "../lib/cronUtils";
import { unreadCount, type NotificationEntry } from "../lib/notificationLog";

export interface NotificationCenterProps {
  entries: NotificationEntry[];
  onMarkAllRead: () => void;
  onClear: () => void;
}

/**
 * Header bell icon + unread badge + dropdown inbox of recent fleet-wide run failures — the
 * persistent, revisitable counterpart to `NotifyToggle`'s transient desktop notification.
 * Closes on outside click or Escape, matching the shell's other dismissible panels.
 */
export function NotificationCenter({ entries, onMarkAllRead, onClear }: NotificationCenterProps) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement>(null);
  const unread = unreadCount(entries);

  useEffect(() => {
    if (!open) return;
    const onDocMouseDown = (e: MouseEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDocMouseDown);
    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("mousedown", onDocMouseDown);
      document.removeEventListener("keydown", onKeyDown);
    };
  }, [open]);

  return (
    <div className="notif-root" ref={rootRef}>
      <button
        type="button"
        className="icon-btn notif-bell"
        title={unread > 0 ? `${unread} unread notification${unread === 1 ? "" : "s"}` : "Notifications"}
        aria-label={unread > 0 ? `${unread} unread notification${unread === 1 ? "" : "s"}` : "Notifications"}
        aria-haspopup="true"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        🔔
        {unread > 0 && <span className="notif-badge">{unread > 99 ? "99+" : unread}</span>}
      </button>
      {open && (
        <div className="notif-panel" role="menu">
          <div className="notif-panel-hd">
            <span className="notif-panel-title">NOTIFICATIONS</span>
            <div className="notif-panel-acts">
              {unread > 0 && (
                <button type="button" className="btn btn-ghost btn-sm" onClick={onMarkAllRead}>
                  MARK READ
                </button>
              )}
              {entries.length > 0 && (
                <button type="button" className="btn btn-ghost btn-sm" onClick={onClear}>
                  CLEAR
                </button>
              )}
            </div>
          </div>
          {entries.length === 0 ? (
            <div className="notif-empty">No notifications yet</div>
          ) : (
            <ul className="notif-list">
              {entries.map((e) => (
                <li key={e.id} className={e.read ? "notif-item" : "notif-item unread"}>
                  <Link to={`/routines?history=${encodeURIComponent(e.routineId)}`} onClick={() => setOpen(false)}>
                    <span className="notif-item-title">{e.routineTitle}</span>
                    <span className="notif-item-msg">{e.message}</span>
                    <span className="notif-item-time">{reltime(e.atSecs)}</span>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
}
