import { Fragment } from "react";
import type { FleetRunSummary, RoutineResponse } from "../../api/hooks";
import { groupRoutines, type RCol, type RDir, type RGroupBy } from "./routineState";
import { RoutineRow } from "./RoutineRow";

function SortTh({
  label,
  col,
  current,
  dir,
  onSort,
}: {
  label: string;
  col: RCol;
  current: RCol | undefined;
  dir: RDir;
  onSort: (col: RCol) => void;
}) {
  const active = current === col;
  const indicator = active ? (dir === "asc" ? " ▲" : " ▼") : "";
  return (
    <th className={active ? "th-sort th-sort-active" : "th-sort"} onClick={() => onSort(col)}>
      {label}
      {indicator}
    </th>
  );
}

export interface RoutineTableProps {
  routines: RoutineResponse[];
  loading: boolean;
  filterActive: boolean;
  now: Date;
  selected: Set<string>;
  onSelect: (id: string) => void;
  onSelectAll: () => void;
  sortCol: RCol | undefined;
  sortDir: RDir;
  groupBy: RGroupBy;
  runHistory: Map<string, FleetRunSummary[]>;
  onSort: (col: RCol) => void;
  onEdit: (id: string) => void;
  onClone: (id: string) => void;
  onDelete: (id: string, title: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
  onLogs: (id: string) => void;
  onHistory: (id: string) => void;
  onFlags: (id: string) => void;
  onClearFilters: () => void;
}

export function RoutineTable(props: RoutineTableProps) {
  const { routines, loading, filterActive, onClearFilters } = props;

  if (loading) {
    return (
      <div className="table-wrap">
        <div className="empty">
          <div className="spinner" />
        </div>
      </div>
    );
  }

  if (routines.length === 0) {
    return (
      <div className="table-wrap">
        {filterActive ? (
          <div className="empty">
            <div className="empty-icon">⊘</div>
            <div className="empty-msg">NO ROUTINES MATCH</div>
            <div className="empty-sub">
              <button type="button" className="btn btn-ghost btn-sm" onClick={onClearFilters}>
                CLEAR FILTERS
              </button>
            </div>
          </div>
        ) : (
          <div className="empty">
            <div className="empty-icon">⧗</div>
            <div className="empty-msg">NO ROUTINES SCHEDULED</div>
            <div className="empty-sub">press + NEW ROUTINE to create one</div>
          </div>
        )}
      </div>
    );
  }

  const allVisibleSelected = routines.length > 0 && routines.every((r) => props.selected.has(r.id));
  const groups = groupRoutines(routines, props.groupBy);

  return (
    <div className="table-wrap">
      <table>
        <thead>
          <tr>
            <th className="col-select">
              <input
                type="checkbox"
                checked={allVisibleSelected}
                onChange={props.onSelectAll}
                aria-label="Select all visible routines"
                title="Select all visible"
              />
            </th>
            <SortTh label="TITLE" col="title" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <th>SCHEDULE</th>
            <SortTh label="NEXT RUN" col="next_run" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <SortTh label="LAST FIRE" col="last_fire" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <th>RUN HISTORY</th>
            <SortTh label="AGENT" col="agent" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <th>REPOS</th>
            <th>MACHINES</th>
            <th>TAGS</th>
            <th>TTL</th>
            <SortTh label="HEALTH" col="health" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <SortTh label="ENABLED" col="enabled" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <SortTh label="UPDATED" col="updated" current={props.sortCol} dir={props.sortDir} onSort={props.onSort} />
            <th />
          </tr>
        </thead>
        <tbody>
          {groups.map(([label, group]) => (
            <Fragment key={`grp-${label}`}>
              {props.groupBy !== "none" && (
                <tr className="group-hd" key={`hd-${label}`}>
                  <td colSpan={15}>
                    <span className="group-label">{label}</span>{" "}
                    <span className="group-count">({group.length})</span>
                  </td>
                </tr>
              )}
              {group.map((r) => (
                <RoutineRow
                  key={r.id}
                  routine={r}
                  now={props.now}
                  runs={props.runHistory.get(r.id) ?? []}
                  selected={props.selected.has(r.id)}
                  onSelect={props.onSelect}
                  onEdit={props.onEdit}
                  onClone={props.onClone}
                  onDelete={props.onDelete}
                  onToggle={props.onToggle}
                  onTrigger={props.onTrigger}
                  onLogs={props.onLogs}
                  onHistory={props.onHistory}
                  onFlags={props.onFlags}
                />
              ))}
            </Fragment>
          ))}
        </tbody>
      </table>
    </div>
  );
}
