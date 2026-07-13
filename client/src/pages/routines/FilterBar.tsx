import type { RefObject } from "react";
import {
  isFilterActive,
  machineFacetValue,
  namedFacetValue,
  parseMachineFacet,
  parseNamedFacet,
  parseStatusFacet,
  type NamedFacet,
  type RoutineFilter,
  type RoutineMachineFacet,
  type RoutineStatusFacet,
} from "./filter";

const STATUS_OPTIONS: [RoutineStatusFacet, string][] = [
  ["all", "All"],
  ["enabled", "Enabled"],
  ["disabled", "Disabled"],
  ["dormant", "Dormant"],
  ["due", "Due soon"],
  ["snoozed", "Snoozed"],
  ["flagged", "Flagged"],
  ["agent-unreg", "Agent unregistered"],
];

export interface FilterBarProps {
  filter: RoutineFilter;
  agents: string[];
  machines: string[];
  repositories: string[];
  tags: string[];
  shown: number;
  total: number;
  searchRef: RefObject<HTMLInputElement>;
  onQuery: (q: string) => void;
  onStatus: (s: RoutineStatusFacet) => void;
  onAgent: (a: NamedFacet) => void;
  onMachine: (m: RoutineMachineFacet) => void;
  onRepository: (r: NamedFacet) => void;
  onTag: (t: NamedFacet) => void;
  onClear: () => void;
}

/** Free-text search + faceted filter controls above the routines table. */
export function FilterBar({
  filter,
  agents,
  machines,
  repositories,
  tags,
  shown,
  total,
  searchRef,
  onQuery,
  onStatus,
  onAgent,
  onMachine,
  onRepository,
  onTag,
  onClear,
}: FilterBarProps) {
  return (
    <div className="filter-bar">
      <div className="filter-field">
        <input
          ref={searchRef}
          type="text"
          className="filter-input"
          placeholder="Search routines…  ( / )"
          aria-label="Search routines"
          value={filter.query}
          onChange={(e) => onQuery(e.target.value)}
        />

        <span className="filter-label">STATUS</span>
        <select
          className="filter-select"
          aria-label="Status filter"
          value={filter.status}
          onChange={(e) => onStatus(parseStatusFacet(e.target.value))}
        >
          {STATUS_OPTIONS.map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </select>

        <span className="filter-label">AGENT</span>
        <select
          className="filter-select"
          aria-label="Agent filter"
          value={namedFacetValue(filter.agent)}
          onChange={(e) => onAgent(parseNamedFacet(e.target.value))}
        >
          <option value={namedFacetValue({ kind: "all" })}>Any</option>
          {agents.map((a) => (
            <option key={a} value={a}>
              {a}
            </option>
          ))}
        </select>

        <span className="filter-label">MACHINE</span>
        <select
          className="filter-select"
          aria-label="Machine filter"
          value={machineFacetValue(filter.machine)}
          onChange={(e) => onMachine(parseMachineFacet(e.target.value))}
        >
          <option value={machineFacetValue({ kind: "any" })}>Any</option>
          <option value={machineFacetValue({ kind: "unassigned" })}>None</option>
          {machines.map((m) => (
            <option key={m} value={m}>
              {m}
            </option>
          ))}
        </select>

        <span className="filter-label">REPOSITORY</span>
        <select
          className="filter-select"
          aria-label="Repository filter"
          value={namedFacetValue(filter.repository)}
          onChange={(e) => onRepository(parseNamedFacet(e.target.value))}
        >
          <option value={namedFacetValue({ kind: "all" })}>Any</option>
          {repositories.map((r) => (
            <option key={r} value={r}>
              {r}
            </option>
          ))}
        </select>

        {tags.length > 0 && (
          <>
            <span className="filter-label">TAG</span>
            <select
              className="filter-select"
              aria-label="Tag filter"
              value={namedFacetValue(filter.tag)}
              onChange={(e) => onTag(parseNamedFacet(e.target.value))}
            >
              <option value={namedFacetValue({ kind: "all" })}>Any</option>
              {tags.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </select>
          </>
        )}
      </div>
      <div className="filter-field">
        <span className="filter-count">
          Showing {shown} of {total}
        </span>
        {isFilterActive(filter) && (
          <button type="button" className="btn btn-ghost btn-sm" title="Clear all filters" onClick={onClear}>
            CLEAR
          </button>
        )}
      </div>
    </div>
  );
}
