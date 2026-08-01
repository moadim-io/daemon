import { useEffect, useRef, useState } from "react";
import { Outlet, useNavigate } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { useAllRuns, useHealth, useMachine, useRenameMachine, useRoutines, useShutdown } from "../api/hooks";
import { freshFailures, snapshotRunStatuses, type RunStatusSnapshot } from "../lib/failureNotify";
import {
  entriesForFailures,
  entriesForMissedScheduledRuns,
  loadNotificationLog,
  markAllRead,
  saveNotificationLog,
  type NotificationEntry,
} from "../lib/notificationLog";
import { applyTheme, loadThemeLight, THEME_CHANGE_EVENT } from "../lib/theme";
import { isEditableTarget, routeForChordKey } from "../lib/keyNav";
import { Header } from "./Header";
import { Nav } from "./Nav";
import { CommandPalette } from "./CommandPalette";
import { RenameMachineDialog } from "./RenameMachineDialog";
import { ShortcutsHelp } from "./ShortcutsHelp";
import { ShutdownDialog } from "./ShutdownDialog";
import { ToastStack } from "./ToastStack";
import { useToasts } from "./toasts";

/** How long a bare `g` keeps the "go-to" chord armed while waiting for its second key. */
const CHORD_TIMEOUT_MS = 900;

/** How many of the most recent fleet-wide runs the always-on notification watch polls. */
const NOTIF_POLL_LIMIT = 50;

/** Notification watch cadence — independent of any page's own (user-configurable) refresh rate. */
const NOTIF_POLL_MS = 15_000;

/** Persistent chrome around every routed page: header, nav, global dialogs, toasts. */
export function Shell() {
  const [lightTheme, setLightTheme] = useState(loadThemeLight);
  const [showShutdown, setShowShutdown] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [showRenameMachine, setShowRenameMachine] = useState(false);
  const [showShortcuts, setShowShortcuts] = useState(false);

  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { addToast } = useToasts();
  const health = useHealth(30_000);
  const machine = useMachine();
  const shutdown = useShutdown();
  const renameMachine = useRenameMachine();
  const notifRuns = useAllRuns(NOTIF_POLL_LIMIT, { refetchInterval: NOTIF_POLL_MS });
  const notifRoutines = useRoutines({ local_only: true }, { refetchInterval: NOTIF_POLL_MS });

  const [notifLog, setNotifLog] = useState<NotificationEntry[]>(loadNotificationLog);
  const prevRunStatuses = useRef<RunStatusSnapshot | null>(null);

  const toggleShortcuts = () => setShowShortcuts((open) => !open);

  // Always-on fleet failure watch, independent of which page is mounted — the persistence layer
  // under the header's Notification Center. Seeds a baseline snapshot on the first tick so
  // failures already in view on load don't flood the inbox; only later transitions are logged.
  useEffect(() => {
    const runsData = notifRuns.data;
    if (runsData === undefined) return;
    if (prevRunStatuses.current === null) {
      prevRunStatuses.current = snapshotRunStatuses(runsData);
      return;
    }
    const failed = freshFailures(runsData, prevRunStatuses.current);
    prevRunStatuses.current = snapshotRunStatuses(runsData);
    if (failed.length === 0) return;
    setNotifLog((prev) => {
      const next = entriesForFailures(prev, failed);
      saveNotificationLog(next);
      return next;
    });
  }, [notifRuns.data]);

  // Missed scheduled fires have no run record, so surface them immediately in the persistent inbox.
  // The backend only exposes an alert timestamp; it does not launch a catch-up run.
  useEffect(() => {
    const routinesData = notifRoutines.data;
    if (routinesData === undefined) return;
    queueMicrotask(() => {
      setNotifLog((prev) => {
        const next = entriesForMissedScheduledRuns(prev, routinesData);
        if (next !== prev) saveNotificationLog(next);
        return next;
      });
    });
  }, [notifRoutines.data]);

  const markNotificationsRead = () => {
    setNotifLog((prev) => {
      const next = markAllRead(prev);
      if (next !== prev) saveNotificationLog(next);
      return next;
    });
  };

  const clearNotifications = () => {
    setNotifLog([]);
    saveNotificationLog([]);
  };

  useEffect(() => {
    applyTheme(lightTheme);
  }, [lightTheme]);

  useEffect(() => {
    const onThemeChange = () => setLightTheme(loadThemeLight());
    window.addEventListener(THEME_CHANGE_EVENT, onThemeChange);
    window.addEventListener("storage", onThemeChange);
    return () => {
      window.removeEventListener(THEME_CHANGE_EVENT, onThemeChange);
      window.removeEventListener("storage", onThemeChange);
    };
  }, []);

  // Global ⌘K / Ctrl-K toggles the palette; `?` toggles the shortcuts cheat
  // sheet; `g` then a bound letter (see keyNav.ts) jumps straight to a page;
  // Escape dismisses whichever shell-level dialog is open. The chord/bare-key
  // handlers bail out while a modifier is held or the event target is a text
  // field, so they never steal keystrokes from routine forms or search boxes.
  useEffect(() => {
    let chordArmed = false;
    let chordTimer: ReturnType<typeof setTimeout> | undefined;
    const disarmChord = () => {
      chordArmed = false;
      clearTimeout(chordTimer);
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setShowPalette((open) => !open);
        return;
      }
      if (event.key === "Escape") {
        setShowShutdown(false);
        setShowRenameMachine(false);
        setShowShortcuts(false);
        disarmChord();
        return;
      }
      if (event.metaKey || event.ctrlKey || event.altKey || isEditableTarget(event.target)) {
        return;
      }
      if (chordArmed) {
        disarmChord();
        const route = routeForChordKey(event.key);
        if (route) {
          event.preventDefault();
          navigate(route);
        }
        return;
      }
      if (event.key.toLowerCase() === "g") {
        chordArmed = true;
        chordTimer = setTimeout(disarmChord, CHORD_TIMEOUT_MS);
      } else if (event.key === "?") {
        event.preventDefault();
        toggleShortcuts();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      clearTimeout(chordTimer);
    };
  }, [navigate]);

  const confirmShutdown = () => {
    setShowShutdown(false);
    shutdown.mutate(undefined, {
      onSuccess: () => {
        addToast("Server stopping…", "ok");
        void queryClient.invalidateQueries({ queryKey: ["health"] });
      },
      onError: (err) => addToast(`Stop failed: ${err.message}`, "err"),
    });
  };

  const confirmRenameMachine = async (name: string) => {
    try {
      await renameMachine.mutateAsync(name);
      setShowRenameMachine(false);
      addToast(`Machine renamed to "${name}"`, "ok");
    } catch (err) {
      addToast(`Rename failed: ${err instanceof Error ? err.message : String(err)}`, "err");
      throw err;
    }
  };

  return (
    <div className="app-shell">
      <Header
        health={health.data}
        healthOk={health.data?.running ?? false}
        machineName={machine.data?.name}
        onRefresh={() => void health.refetch()}
        onStop={() => setShowShutdown(true)}
        onPalette={() => setShowPalette((open) => !open)}
        onRenameMachine={() => setShowRenameMachine(true)}
        onShortcuts={toggleShortcuts}
        notifications={{ entries: notifLog, onMarkAllRead: markNotificationsRead, onClear: clearNotifications }}
      />
      <Nav />
      <div className="page">
        <Outlet />
      </div>
      <CommandPalette
        open={showPalette}
        onClose={() => setShowPalette(false)}
        onRefresh={() => void health.refetch()}
        onStop={() => setShowShutdown(true)}
        onShortcuts={toggleShortcuts}
      />
      {showShortcuts && <ShortcutsHelp onClose={() => setShowShortcuts(false)} />}
      {showRenameMachine && (
        <RenameMachineDialog
          current={machine.data?.name ?? ""}
          onCancel={() => setShowRenameMachine(false)}
          onConfirm={confirmRenameMachine}
        />
      )}
      {showShutdown && (
        <ShutdownDialog onCancel={() => setShowShutdown(false)} onConfirm={confirmShutdown} />
      )}
      <ToastStack />
    </div>
  );
}
