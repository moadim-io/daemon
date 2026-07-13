import { groupByLabel, parseRGroupBy, type RGroupBy } from "./routineState";

const GROUP_BYS: readonly RGroupBy[] = ["none", "agent", "machine", "status", "health"];

export interface GroupBySelectorProps {
  groupBy: RGroupBy;
  onChange: (by: RGroupBy) => void;
}

/** Dropdown to partition the routine table by agent/machine/status/health. */
export function GroupBySelector({ groupBy, onChange }: GroupBySelectorProps) {
  return (
    <div className="group-by-ctrl">
      <label htmlFor="routine-group-by-select">GROUP BY</label>
      <select
        id="routine-group-by-select"
        className="filter-select"
        aria-label="Group routines by"
        value={groupBy}
        onChange={(e) => onChange(parseRGroupBy(e.target.value))}
      >
        {GROUP_BYS.map((by) => (
          <option key={by} value={by}>
            {groupByLabel(by)}
          </option>
        ))}
      </select>
    </div>
  );
}
