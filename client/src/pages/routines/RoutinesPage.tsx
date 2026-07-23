/**
 * Top-level Routines page — composition of the filter/table/calendar/day views, the
 * create/edit form, and the history/logs/flags sub-pages.
 *
 * `RPage`/`RModal` state modeling: the Rust source (`ui/src/routines/state.rs (removed)`) models this via
 * a `RPage` enum (List/New/Logs(id)/History(id)/Flags(id)/Clone(Routine)) plus an independent
 * `RModal` overlay (None/Edit(id)/ConfirmDelete/ConfirmBulkDelete). This port keeps that same
 * shape as plain `useState` rather than nested routes: every sub-page needs the already-loaded
 * `routines` list (for title lookups, the clone source, etc.), and nested routes would need that
 * list threaded through a route loader or context for no real benefit — a query param round-trip
 * to the router buys us nothing a local discriminated-union state doesn't already give simply.
 * The one exception is the `?history=<id>` deep link from the Overview page's recent-runs panel,
 * which IS a URL param (read once on mount, then behaves like any other page transition).
 */
import { useEffect, useMemo, useRef, useState } from "react";
import { useSearchParams } from "react-router-dom";
import {
  useAllRuns,
  useCleanupRoutines,
  useCreateRoutine,
  useDeleteRoutine,
  useLockStatus,
  useMachine,
  useMachines,
  useRoutine,
  useRoutines,
  useTriggerRoutine,
  useUnlock,
  useUpdateRoutine,
  type RoutineResponse,
} from "../../api/hooks";
import { GlobalLockBanner } from "../../components/GlobalLockBanner";
import { loadRefreshToken, RefreshControl, refreshMs, saveRefreshToken, type RefreshToken } from "../../components/RefreshControl";
import { useToasts } from "../../shell/toasts";
import { BulkBar, BulkDeleteDialog, ConfirmDeleteDialog } from "./BulkBar";
import { humanizeBytes } from "./bytes";
import { downloadCsv, routinesToCsv } from "./csvExport";
import { DayTimeline, type TimelineItem } from "./DayTimeline";
import {
  DUE_SOON_WINDOW_MS,
  defaultRoutineFilter,
  distinctAgents,
  distinctMachines,
  distinctRepositories,
  distinctTags,
  filterRoutines,
  isFilterActive,
  isRoutineSnoozed,
  type NamedFacet,
  type RoutineFilter,
  type RoutineMachineFacet,
  type RoutineStatusFacet,
} from "./filter";
import { FilterBar } from "./FilterBar";
import { GroupBySelector } from "./GroupBySelector";
import { RoutineCalendar } from "./RoutineCalendar";
import { RoutineFlags } from "./RoutineFlags";
import { RoutineForm, type RoutineDraft } from "./RoutineForm";
import { RoutineHistory } from "./RoutineHistory";
import { RoutineLogs } from "./RoutineLogs";
import { RoutineTable } from "./RoutineTable";
import { cloneTitle } from "./routineDraft";
import { flipDir, sortRoutines, type RCol, type RDir, type RGroupBy } from "./routineState";
import {
  captureSnapshot,
  decodeSnapshot,
  loadLastView,
  loadSavedViews,
  saveLastView,
  saveSavedViews,
  type SavedView,
  type ViewSnapshot,
} from "./savedViews";
import { SavedViewsBar } from "./SavedViewsBar";
import { groupRecentRuns, RUN_HISTORY_FETCH_LIMIT } from "./sparkline";
import { StatsBar } from "./StatsBar";
import { ViewToggle, type RView } from "./ViewToggle";

type RPage =
  | { kind: "list" }
  | { kind: "new" }
  | { kind: "clone"; source: RoutineResponse }
  | { kind: "logs"; id: string }
  | { kind: "history"; id: string }
  | { kind: "flags"; id: string };

type RModal =
  | { kind: "none" }
  | { kind: "edit"; id: string }
  | { kind: "confirmDelete"; id: string; title: string }
  | { kind: "confirmBulkDelete" };

const NEXT_RUN_TICK_MS = 30_000;

export function RoutinesPage() {
  const { addToast } = useToasts();
  const [searchParams, setSearchParams] = useSearchParams();

  // ── Persisted view state (restored from the last-used filter/sort/group-by) ──
  const initialSnapshot = useMemo(() => loadLastView(), []);
  const initialDecoded = initialSnapshot ? decodeSnapshot(initialSnapshot) : undefined;

  // Deep link: `/routines?history=<id>` (e.g. from the Overview page's recent-runs
  // panel) picks the initial page via a lazy initializer rather than an
  // effect + setState, since it only ever matters for the very first render.
  const [page, setPage] = useState<RPage>((): RPage => {
    const id = searchParams.get("history");
    return id ? { kind: "history", id } : { kind: "list" };
  });
  const [modal, setModal] = useState<RModal>({ kind: "none" });
  const [view, setView] = useState<RView>("table");
  const [filter, setFilter] = useState<RoutineFilter>(initialDecoded?.filter ?? defaultRoutineFilter());
  const [sortCol, setSortCol] = useState<RCol | undefined>(initialDecoded?.sortCol);
  const [sortDir, setSortDir] = useState<RDir>(initialDecoded?.sortDir ?? "asc");
  const [groupBy, setGroupBy] = useState<RGroupBy>(initialDecoded?.groupBy ?? "none");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [savedViewsList, setSavedViewsList] = useState<SavedView[]>(loadSavedViews);
  const [interval, setIntervalState] = useState<RefreshToken>(loadRefreshToken);
  const [now, setNow] = useState(() => new Date());
  const searchRef = useRef<HTMLInputElement>(null);

  // Strip the `history` param from the URL once consumed above, so a reload or
  // share link doesn't re-trigger the deep link.
  useEffect(() => {
    if (searchParams.get("history")) {
      const next = new URLSearchParams(searchParams);
      next.delete("history");
      setSearchParams(next, { replace: true });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Live "now" clock, ticked independently of any network fetch.
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), NEXT_RUN_TICK_MS);
    return () => clearInterval(id);
  }, []);

  // Auto-persist the current filter/sort/group-by so a reload restores it.
  useEffect(() => {
    saveLastView(captureSnapshot(filter, sortCol, sortDir, groupBy));
  }, [filter, sortCol, sortDir, groupBy]);

  // Global "/" focuses search, Escape closes the open modal.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape" && modal.kind !== "none") {
        setModal({ kind: "none" });
        return;
      }
      const target = e.target as HTMLElement | null;
      const typing = target && ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
      if (e.key === "/" && !typing && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [modal.kind]);

  const refetchMs = refreshMs(interval);
  const routinesQuery = useRoutines({}, { refetchInterval: refetchMs });
  const allRunsQuery = useAllRuns(RUN_HISTORY_FETCH_LIMIT);
  const lockStatusQuery = useLockStatus();
  const currentMachineQuery = useMachine();
  const machinesQuery = useMachines();

  // Re-arm the fleet-run-history poll on the same cadence as the routines list.
  useEffect(() => {
    if (refetchMs === undefined) return;
    const id = setInterval(() => void allRunsQuery.refetch(), refetchMs);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [refetchMs]);

  const routines = useMemo(() => routinesQuery.data ?? [], [routinesQuery.data]);
  const runHistory = useMemo(() => groupRecentRuns(allRunsQuery.data ?? []), [allRunsQuery.data]);

  // Default the machine facet to this daemon's own identity, once, unless a restored saved view
  // (or a fast operator) already picked something.
  const defaultedMachineRef = useRef(false);
  useEffect(() => {
    const name = currentMachineQuery.data?.name;
    if (!name || defaultedMachineRef.current) return;
    defaultedMachineRef.current = true;
    setFilter((f) => (f.machine.kind === "any" ? { ...f, machine: { kind: "machine", value: name } } : f));
  }, [currentMachineQuery.data]);

  // Drop selections for routines that no longer exist after a reload. Adjusted
  // during render (guarded on the `routines` reference changing) rather than in
  // an effect — see the `seededFrom` comment in SettingsPage.tsx for the pattern.
  const [prevRoutines, setPrevRoutines] = useState(routines);
  if (routines !== prevRoutines) {
    setPrevRoutines(routines);
    const ids = new Set(routines.map((r) => r.id));
    setSelected((sel) => {
      const next = new Set([...sel].filter((id) => ids.has(id)));
      return next.size === sel.size ? sel : next;
    });
  }

  // ── Mutations ──────────────────────────────────────────────────────────────
  const createRoutine = useCreateRoutine();
  const updateRoutine = useUpdateRoutine();
  const deleteRoutine = useDeleteRoutine();
  const triggerRoutine = useTriggerRoutine();
  const cleanupRoutines = useCleanupRoutines();
  const unlock = useUnlock();

  const goToList = () => setPage({ kind: "list" });
  const closeModal = () => setModal({ kind: "none" });

  const onUnlockAll = () => {
    unlock.mutate("all", {
      onSuccess: () => addToast("Routines unlocked", "ok"),
      onError: (e) => addToast(`Unlock failed: ${e.message}`, "err"),
    });
  };

  const onCreate = (draft: RoutineDraft) => {
    createRoutine.mutate(
      { ...draft },
      {
        onSuccess: () => {
          goToList();
          addToast("Routine created", "ok");
        },
        onError: (e) => addToast(`Create failed: ${e.message}`, "err"),
      },
    );
  };

  const onSaveEdit = (id: string, draft: RoutineDraft) => {
    updateRoutine.mutate(
      { id, body: { ...draft } },
      {
        onSuccess: () => {
          closeModal();
          addToast("Routine updated", "ok");
        },
        onError: (e) => addToast(`Update failed: ${e.message}`, "err"),
      },
    );
  };

  const onConfirmDelete = (id: string) => {
    deleteRoutine.mutate(id, {
      onSuccess: () => {
        closeModal();
        addToast("Routine deleted", "ok");
      },
      onError: (e) => addToast(`Delete failed: ${e.message}`, "err"),
    });
  };

  const onToggle = (id: string, enabled: boolean) => {
    updateRoutine.mutate(
      { id, body: { enabled } },
      {
        onSuccess: () => addToast(enabled ? "Routine enabled" : "Routine disabled", "ok"),
        onError: (e) => addToast(`Toggle failed: ${e.message}`, "err"),
      },
    );
  };

  const onTrigger = (id: string) => {
    triggerRoutine.mutate(id, {
      onSuccess: () => addToast("Routine triggered", "ok"),
      onError: (e) => addToast(`Trigger failed: ${e.message}`, "err"),
    });
  };

  const onCleanup = () => {
    cleanupRoutines.mutate(undefined, {
      onSuccess: (res) => {
        const n = res.removed;
        addToast(
          `Cleanup removed ${n} workbench${n === 1 ? "" : "es"} (freed ${humanizeBytes(res.freed_bytes)})`,
          "ok",
        );
      },
      onError: (e) => addToast(`Cleanup failed: ${e.message}`, "err"),
    });
  };

  // ── Bulk actions (sequential, per-item — no batch endpoint) ─────────────────
  const bulkSetEnabled = async (enabled: boolean) => {
    const ids = [...selected];
    if (ids.length === 0) return;
    let ok = 0;
    let failed = 0;
    for (const id of ids) {
      try {
        await updateRoutine.mutateAsync({ id, body: { enabled } });
        ok++;
      } catch {
        failed++;
      }
    }
    const verb = enabled ? "enabled" : "disabled";
    if (failed === 0) addToast(`${ok} routine(s) ${verb}`, "ok");
    else addToast(`${ok} ${verb}, ${failed} failed`, "err");
  };

  const onConfirmBulkDelete = async () => {
    const ids = [...selected];
    let ok = 0;
    let failed = 0;
    const deleted: string[] = [];
    for (const id of ids) {
      try {
        await deleteRoutine.mutateAsync(id);
        ok++;
        deleted.push(id);
      } catch {
        failed++;
      }
    }
    setSelected((sel) => {
      const next = new Set(sel);
      for (const id of deleted) next.delete(id);
      return next;
    });
    closeModal();
    if (failed === 0) addToast(`${ok} routine(s) deleted`, "ok");
    else addToast(`${ok} deleted, ${failed} failed`, "err");
  };

  // ── Saved views ──────────────────────────────────────────────────────────────
  const onApplyView = (snapshot: ViewSnapshot) => {
    const decoded = decodeSnapshot(snapshot);
    setFilter(decoded.filter);
    setSortCol(decoded.sortCol);
    setSortDir(decoded.sortDir);
    setGroupBy(decoded.groupBy);
  };
  const onSaveView = (name: string) => {
    const snapshot = captureSnapshot(filter, sortCol, sortDir, groupBy);
    setSavedViewsList((list) => {
      const next = [...list.filter((v) => v.name !== name), { name, snapshot }];
      saveSavedViews(next);
      return next;
    });
  };
  const onDeleteView = (name: string) => {
    setSavedViewsList((list) => {
      const next = list.filter((v) => v.name !== name);
      saveSavedViews(next);
      return next;
    });
  };

  // ── Derived data ──────────────────────────────────────────────────────────────
  const agentOptions = useMemo(() => distinctAgents(routines), [routines]);
  const repositoryOptions = useMemo(() => distinctRepositories(routines), [routines]);
  const tagOptions = useMemo(() => distinctTags(routines), [routines]);
  const machineOptions = useMemo(() => {
    const opts = new Set([...distinctMachines(routines), ...(machinesQuery.data ?? [])]);
    return [...opts].sort();
  }, [routines, machinesQuery.data]);

  const filterActive = isFilterActive(filter);
  const visible = useMemo(
    () => sortRoutines(filterRoutines(routines, filter, now, DUE_SOON_WINDOW_MS), sortCol, sortDir, now),
    [routines, filter, now, sortCol, sortDir],
  );

  const onExportCsv = () => {
    downloadCsv(`routines-${new Date().toISOString().slice(0, 10)}.csv`, routinesToCsv(visible));
    addToast(`Exported ${visible.length} routine(s) to CSV`, "ok");
  };

  // List rows omit `prompt` by default (see `include_prompts`), so the edit form fetches the
  // full routine by id instead of reusing the cached list row.
  const editRoutineQuery = useRoutine(modal.kind === "edit" ? modal.id : "", modal.kind === "edit");
  const editRoutine = modal.kind === "edit" ? editRoutineQuery.data : undefined;

  const onSelect = (id: string) =>
    setSelected((sel) => {
      const next = new Set(sel);
      if (!next.delete(id)) next.add(id);
      return next;
    });
  const onSelectAll = () => {
    const visibleIds = visible.map((r) => r.id);
    const allSelected = visibleIds.length > 0 && visibleIds.every((id) => selected.has(id));
    setSelected(allSelected ? new Set() : new Set(visibleIds));
  };

  const onColSort = (col: RCol) => {
    if (sortCol === col) setSortDir((d) => flipDir(d));
    else {
      setSortCol(col);
      setSortDir("asc");
    }
  };

  const onSetInterval = (next: RefreshToken) => {
    setIntervalState(next);
    saveRefreshToken(next);
  };

  const titleOf = (id: string) => routines.find((r) => r.id === id)?.title ?? "";

  // ── Sub-pages ──────────────────────────────────────────────────────────────
  if (page.kind === "new") {
    return <RoutineForm mode="create" saving={createRoutine.isPending} onCancel={goToList} onSave={onCreate} />;
  }
  if (page.kind === "clone") {
    const pre: RoutineDraft = {
      schedule: page.source.schedule,
      title: cloneTitle(page.source.title),
      agent: page.source.agent,
      model: page.source.model ?? null,
      prompt: page.source.prompt ?? "",
      goal: page.source.goal ?? null,
      repositories: page.source.repositories ?? [],
      machines: page.source.machines ?? [],
      enabled: page.source.enabled,
      ttl_secs: page.source.ttl_secs ?? null,
      tags: page.source.tags ?? [],
    };
    return (
      <RoutineForm mode="clone" initial={pre} saving={createRoutine.isPending} onCancel={goToList} onSave={onCreate} />
    );
  }
  if (page.kind === "logs") {
    return <RoutineLogs id={page.id} title={titleOf(page.id)} onBack={goToList} />;
  }
  if (page.kind === "history") {
    return <RoutineHistory id={page.id} title={titleOf(page.id)} onBack={goToList} />;
  }
  if (page.kind === "flags") {
    return <RoutineFlags id={page.id} title={titleOf(page.id)} onBack={goToList} />;
  }

  // ── List page ──────────────────────────────────────────────────────────────
  const dayItems: TimelineItem[] = visible
    .filter((r) => r.enabled)
    .map((r) => ({
      id: r.id,
      label: r.title,
      schedule: r.schedule,
      snoozed: isRoutineSnoozed(r, now),
      flagCount: r.flag_count ?? 0,
    }));

  return (
    <div className="page">
      <h1 className="page-title">Routines</h1>
      <GlobalLockBanner status={lockStatusQuery.data} onUnlock={onUnlockAll} />
      <StatsBar
        routines={routines}
        now={now}
        active={filter.status}
        onStatus={(s: RoutineStatusFacet) => setFilter((f) => ({ ...f, status: s }))}
      />

      <div className="section-hd">
        <div className="section-label">SCHEDULED ROUTINES</div>
        <div className="section-acts">
          <RefreshControl token={interval} updatedAtMs={routinesQuery.dataUpdatedAt} onChange={onSetInterval} />
          {view === "table" && <GroupBySelector groupBy={groupBy} onChange={setGroupBy} />}
          <ViewToggle view={view} onSetView={setView} />
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            title="Reap finished, expired run workbenches now"
            onClick={onCleanup}
          >
            CLEANUP NOW
          </button>
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            title="Export the currently filtered/sorted routines to a CSV file"
            disabled={visible.length === 0}
            onClick={onExportCsv}
          >
            EXPORT CSV
          </button>
          <button type="button" className="btn btn-primary btn-sm" onClick={() => setPage({ kind: "new" })}>
            + NEW ROUTINE
          </button>
        </div>
      </div>

      <FilterBar
        filter={filter}
        agents={agentOptions}
        machines={machineOptions}
        repositories={repositoryOptions}
        tags={tagOptions}
        shown={visible.length}
        total={routines.length}
        searchRef={searchRef}
        onQuery={(q) => setFilter((f) => ({ ...f, query: q }))}
        onStatus={(s) => setFilter((f) => ({ ...f, status: s }))}
        onAgent={(a: NamedFacet) => setFilter((f) => ({ ...f, agent: a }))}
        onMachine={(m: RoutineMachineFacet) => setFilter((f) => ({ ...f, machine: m }))}
        onRepository={(r: NamedFacet) => setFilter((f) => ({ ...f, repository: r }))}
        onTag={(t: NamedFacet) => setFilter((f) => ({ ...f, tag: t }))}
        onClear={() => setFilter(defaultRoutineFilter())}
      />

      <SavedViewsBar views={savedViewsList} onApply={onApplyView} onSave={onSaveView} onDelete={onDeleteView} />

      <BulkBar
        count={selected.size}
        onEnable={() => void bulkSetEnabled(true)}
        onDisable={() => void bulkSetEnabled(false)}
        onDelete={() => setModal({ kind: "confirmBulkDelete" })}
        onClear={() => setSelected(new Set())}
      />

      {view === "table" && (
        <RoutineTable
          routines={visible}
          loading={routinesQuery.isLoading}
          filterActive={filterActive}
          now={now}
          selected={selected}
          onSelect={onSelect}
          onSelectAll={onSelectAll}
          sortCol={sortCol}
          sortDir={sortDir}
          groupBy={groupBy}
          runHistory={runHistory}
          onSort={onColSort}
          onEdit={(id) => setModal({ kind: "edit", id })}
          onClone={(id) => {
            const source = routines.find((r) => r.id === id);
            if (source) setPage({ kind: "clone", source });
          }}
          onDelete={(id, title) => setModal({ kind: "confirmDelete", id, title })}
          onToggle={onToggle}
          onTrigger={onTrigger}
          onLogs={(id) => setPage({ kind: "logs", id })}
          onHistory={(id) => setPage({ kind: "history", id })}
          onFlags={(id) => setPage({ kind: "flags", id })}
          onClearFilters={() => setFilter(defaultRoutineFilter())}
        />
      )}
      {view === "calendar" && (
        <RoutineCalendar
          routines={visible}
          loading={routinesQuery.isLoading}
          onEdit={(id) => setModal({ kind: "edit", id })}
        />
      )}
      {view === "day" && (
        <DayTimeline
          items={dayItems}
          loading={routinesQuery.isLoading}
          onClick={(id) => setModal({ kind: "edit", id })}
        />
      )}

      {modal.kind === "edit" && editRoutineQuery.isLoading && (
        <div className="empty">
          <div className="spinner" />
        </div>
      )}
      {modal.kind === "edit" && editRoutine && (
        <RoutineForm
          mode="edit"
          initial={
            editRoutine && {
              schedule: editRoutine.schedule,
              title: editRoutine.title,
              agent: editRoutine.agent,
              model: editRoutine.model ?? null,
              prompt: editRoutine.prompt ?? "",
              goal: editRoutine.goal ?? null,
              repositories: editRoutine.repositories ?? [],
              machines: editRoutine.machines ?? [],
              enabled: editRoutine.enabled,
              ttl_secs: editRoutine.ttl_secs ?? null,
              tags: editRoutine.tags ?? [],
            }
          }
          saving={updateRoutine.isPending}
          onCancel={closeModal}
          onSave={(draft) => onSaveEdit(modal.id, draft)}
        />
      )}
      {modal.kind === "confirmDelete" && (
        <ConfirmDeleteDialog title={modal.title} onCancel={closeModal} onConfirm={() => onConfirmDelete(modal.id)} />
      )}
      {modal.kind === "confirmBulkDelete" && (
        <BulkDeleteDialog count={selected.size} onCancel={closeModal} onConfirm={() => void onConfirmBulkDelete()} />
      )}
    </div>
  );
}
