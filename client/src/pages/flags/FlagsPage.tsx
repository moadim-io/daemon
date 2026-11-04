import { useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { useFleetFlags, useResolveFlag, useRoutines } from "../../api/hooks";
import { RefreshFreshness, refreshMs, useRefreshToken } from "../../components/RefreshControl";
import { SortableTh } from "../../components/SortableTh";
import { abstime, reltime } from "../../lib/cronUtils";
import { sortRows, usePersistedSort, type SortAccessors } from "../../lib/tableSort";
import { buildFlagRows, filterFlagRows, flagStats, flagTypes, type FlagRow } from "./flagsInbox";

type Col = "age" | "routine" | "type";
const COLS: readonly Col[] = ["age", "routine", "type"];
const ACCESSORS: SortAccessors<FlagRow, Col> = {
  age: (r) => r.flag.created_at,
  routine: (r) => r.routineTitle,
  type: (r) => r.flag.type,
};

/**
 * The FLAGS page: a fleet-wide triage inbox of every open flag agents raised, across all
 * routines, oldest first — "what is waiting on me?" in one sortable, searchable list with an
 * inline RESOLVE, instead of opening each flagged routine's panel one by one.
 */
export function FlagsPage() {
  const ms = refreshMs(useRefreshToken());
  const routinesQuery = useRoutines({}, { refetchInterval: ms });
  const flagged = useMemo(() => (routinesQuery.data ?? []).filter((r) => r.flag_count > 0), [routinesQuery.data]);
  const flagQueries = useFleetFlags(
    flagged.map((r) => r.id),
    ms,
  );
  const resolveFlag = useResolveFlag();
  const { sort, toggle } = usePersistedSort<Col>("moadim.flags.sort", COLS);
  const [query, setQuery] = useState("");
  const [type, setType] = useState("");

  const rows = buildFlagRows(
    flagged,
    flagQueries.map((q) => q.data),
  );
  const shown = sortRows(filterFlagRows(rows, query, type), sort, ACCESSORS);
  const stats = flagStats(rows);

  const isLoading = routinesQuery.isLoading || flagQueries.some((q) => q.isLoading);
  const errorMessage = routinesQuery.error?.message ?? flagQueries.find((q) => q.error)?.error?.message;
  const updatedAtMs = Math.min(routinesQuery.dataUpdatedAt || Infinity, ...flagQueries.map((q) => q.dataUpdatedAt || Infinity));

  return (
    <div className="page">
      <div className="section-hd">
        <h1 className="page-title">Flags</h1>
        <div className="section-acts">
          <RefreshFreshness updatedAtMs={Number.isFinite(updatedAtMs) ? updatedAtMs : 0} />
        </div>
      </div>

      <div className="stats">
        <div className="stat-card">
          <div className="stat-label">OPEN FLAGS</div>
          <div className={stats.open > 0 ? "stat-val c-red" : "stat-val"}>{stats.open}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">ROUTINES FLAGGED</div>
          <div className="stat-val">{stats.routines}</div>
        </div>
        <div className="stat-card">
          <div className="stat-label">OLDEST FLAG</div>
          <div className="stat-val">{stats.oldestAt === null ? "—" : reltime(stats.oldestAt)}</div>
        </div>
      </div>

      {errorMessage ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⚠</div>
            <div className="empty-msg">FAILED TO LOAD</div>
            <div className="empty-sub">{errorMessage}</div>
          </div>
        </div>
      ) : isLoading ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="spinner" />
          </div>
        </div>
      ) : rows.length === 0 ? (
        <div className="table-wrap">
          <div className="empty">
            <div className="empty-icon">⚑</div>
            <div className="empty-msg">INBOX ZERO</div>
            <div className="empty-sub">no routine has an open flag</div>
          </div>
        </div>
      ) : (
        <>
          <div className="filter-bar">
            <input
              type="search"
              className="search-input"
              placeholder="Search flags…"
              aria-label="Search flags"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
            <select className="filter-select" aria-label="Flag type" value={type} onChange={(e) => setType(e.target.value)}>
              <option value="">all types</option>
              {flagTypes(rows).map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
            <span className="cell-meta">
              {shown.length} of {rows.length}
            </span>
          </div>
          <div className="table-wrap">
            {shown.length === 0 ? (
              <div className="empty">
                <div className="empty-msg">NO MATCHING FLAGS</div>
              </div>
            ) : (
              <table>
                <thead>
                  <tr>
                    <SortableTh label="AGE" col="age" sort={sort} defaultDir="asc" onSort={toggle} />
                    <SortableTh label="ROUTINE" col="routine" sort={sort} defaultDir="asc" onSort={toggle} />
                    <SortableTh label="TYPE" col="type" sort={sort} defaultDir="asc" onSort={toggle} />
                    <th>SCOPE</th>
                    <th>DESCRIPTION</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {shown.map((r) => (
                    <tr key={`${r.routineId}/${r.flag.filename}`}>
                      <td className="cell-meta" title={abstime(r.flag.created_at)}>
                        {reltime(r.flag.created_at)}
                      </td>
                      <td>
                        <Link to={`/routines?history=${encodeURIComponent(r.routineId)}`}>{r.routineTitle}</Link>
                      </td>
                      <td>
                        <span className="flag-type">{r.flag.type}</span>
                      </td>
                      <td className="cell-meta">{r.flag.scope === "general" ? "general" : "local"}</td>
                      <td className="flag-desc">{r.flag.description}</td>
                      <td>
                        <button
                          type="button"
                          className="btn btn-ghost btn-sm"
                          disabled={resolveFlag.isPending}
                          aria-label={`Resolve ${r.flag.type} flag on ${r.routineTitle}`}
                          onClick={() => resolveFlag.mutate({ id: r.routineId, filename: r.flag.filename })}
                        >
                          RESOLVE
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </>
      )}
    </div>
  );
}
