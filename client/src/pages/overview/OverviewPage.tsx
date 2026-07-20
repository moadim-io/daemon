import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAllRuns, useLockStatus, useRoutines, useTriggerRoutine, useUnlock } from "../../api/hooks";
import { GlobalLockBanner } from "../../components/GlobalLockBanner";
import { loadRefreshToken, refreshMs, RefreshControl, saveRefreshToken, type RefreshToken } from "../../components/RefreshControl";
import {
  fireFailureNotification,
  freshFailures,
  loadNotifyFailures,
  requestNotifyPermission,
  saveNotifyFailures,
  snapshotRunStatuses,
  type RunStatusSnapshot,
} from "../../lib/failureNotify";
import { useToasts } from "../../shell/toasts";
import { AttentionTable } from "./AttentionTable";
import { NotifyToggle } from "./NotifyToggle";
import { attentionItems, computeKpis, nextRunSummary, sourcesOf, upcomingRuns } from "./overviewLogic";
import { OverviewStats } from "./OverviewStats";
import { RecentRunsTable } from "./RecentRunsTable";
import { UpcomingTable } from "./UpcomingTable";

/** How many of the most recent runs across the fleet the overview panel shows. */
const RECENT_RUNS_LIMIT = 8;

/** How often the live "now" advances so countdowns re-render between fetches. */
const TICK_MS = 10_000;

/**
 * The OVERVIEW landing page: a single-pane operations summary that
 * aggregates routines into KPI tiles and one merged "upcoming runs"
 * schedule. Direct port of `ui/src/overview.rs`'s `OverviewPage` shell — all
 * KPI/merge/attention math lives in the host-tested `overviewLogic.ts`; this
 * component is a thin shell that maps fetched records into `SchedSource`s
 * and renders the result.
 */
export function OverviewPage() {
  const queryClient = useQueryClient();
  const { addToast } = useToasts();

  const routinesQuery = useRoutines();
  const runsQuery = useAllRuns(RECENT_RUNS_LIMIT);
  const lockQuery = useLockStatus();
  const unlock = useUnlock();
  const trigger = useTriggerRoutine();

  const [now, setNow] = useState(() => new Date());
  const [refreshToken, setRefreshToken] = useState<RefreshToken>(loadRefreshToken);
  const [notifyEnabled, setNotifyEnabled] = useState(loadNotifyFailures);
  const prevRunStatuses = useRef<RunStatusSnapshot | null>(null);

  // Advance "now" so countdowns re-render between fetches.
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), TICK_MS);
    return () => clearInterval(id);
  }, []);

  // Auto-refresh loop, re-armed when the interval changes. A single
  // invalidation covers routines, lock status, and recent runs — all three
  // hooks share the "routines" query-key prefix.
  useEffect(() => {
    const ms = refreshMs(refreshToken);
    if (ms === undefined) return;
    const id = setInterval(() => {
      void queryClient.invalidateQueries({ queryKey: ["routines"] });
    }, ms);
    return () => clearInterval(id);
  }, [refreshToken, queryClient]);

  const handleSetRefreshToken = useCallback((token: RefreshToken) => {
    saveRefreshToken(token);
    setRefreshToken(token);
  }, []);

  // Watches each poll of the fleet's recent runs for fresh failures and fires a desktop
  // notification per run. Seeds a baseline snapshot on the first tick after enabling so
  // failures already in view don't spam on toggle-on — only later transitions surface.
  useEffect(() => {
    if (!notifyEnabled) {
      prevRunStatuses.current = null;
      return;
    }
    const runsData = runsQuery.data;
    if (runsData === undefined) return;
    if (prevRunStatuses.current === null) {
      prevRunStatuses.current = snapshotRunStatuses(runsData);
      return;
    }
    for (const failed of freshFailures(runsData, prevRunStatuses.current)) {
      fireFailureNotification(failed);
    }
    prevRunStatuses.current = snapshotRunStatuses(runsData);
  }, [runsQuery.data, notifyEnabled]);

  const handleToggleNotify = useCallback(
    (next: boolean) => {
      if (!next) {
        saveNotifyFailures(false);
        setNotifyEnabled(false);
        return;
      }
      void requestNotifyPermission().then((perm) => {
        if (perm === "granted") {
          saveNotifyFailures(true);
          setNotifyEnabled(true);
        } else {
          addToast(
            perm === "unsupported"
              ? "Desktop notifications aren't supported in this browser"
              : "Notification permission was denied",
            "err",
          );
        }
      });
    },
    [addToast],
  );

  const handleTrigger = useCallback(
    (id: string) => {
      trigger.mutate(id, {
        onSuccess: () => addToast("Triggered", "ok"),
        onError: (err) => addToast(`Trigger failed: ${err instanceof Error ? err.message : String(err)}`, "err"),
      });
    },
    [trigger, addToast],
  );

  const handleUnlock = useCallback(() => {
    unlock.mutate("all");
  }, [unlock]);

  const sources = sourcesOf(routinesQuery.data ?? [], now);
  const kpis = computeKpis(sources, now);
  const attention = attentionItems(sources, now);
  const runs = upcomingRuns(sources, now);
  const nextRun = nextRunSummary(runs, now);
  const updatedAtMs = Math.max(routinesQuery.dataUpdatedAt, runsQuery.dataUpdatedAt, lockQuery.dataUpdatedAt);
  const loadError = routinesQuery.error instanceof Error ? routinesQuery.error.message : undefined;

  return (
    <div>
      <h1 className="page-title">Overview</h1>
      <GlobalLockBanner status={lockQuery.data} onUnlock={handleUnlock} />
      <OverviewStats kpis={kpis} nextRun={nextRun} />
      {attention.length > 0 && (
        <>
          <div className="section-hd">
            <span className="section-label attn">NEEDS ATTENTION</span>
          </div>
          <AttentionTable items={attention} />
        </>
      )}
      <div className="section-hd">
        <span className="section-label">UPCOMING RUNS</span>
        <div className="section-acts">
          <RefreshControl token={refreshToken} updatedAtMs={updatedAtMs} onChange={handleSetRefreshToken} />
        </div>
      </div>
      <UpcomingTable runs={runs} now={now} loading={routinesQuery.isLoading} error={loadError} onTrigger={handleTrigger} />
      <div className="section-hd">
        <span className="section-label">RECENT RUNS</span>
        <div className="section-acts">
          <NotifyToggle enabled={notifyEnabled} onToggle={handleToggleNotify} />
        </div>
      </div>
      <RecentRunsTable runs={runsQuery.data ?? []} loading={runsQuery.isLoading} />
    </div>
  );
}
