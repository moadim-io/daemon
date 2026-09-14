import { useEffect, useRef } from "react";
import { machineFacetValue, parseMachineFacet, type RoutineMachineFacet } from "./filter";
import "./MachineFilter.css";

/** Native disclosure and checkboxes work with touch, keyboard, and screen readers. */
export function MachineFilter({ facet, machines, onChange }: {
  facet: RoutineMachineFacet;
  machines: string[];
  onChange: (facet: RoutineMachineFacet) => void;
}) {
  const disclosure = useRef<HTMLDetailsElement>(null);
  const summary = useRef<HTMLElement>(null);
  useEffect(() => {
    const onPointerDown = (event: PointerEvent) => {
      const details = disclosure.current;
      if (details?.open && event.target instanceof Node && !details.contains(event.target)) {
        details.open = false;
      }
    };
    document.addEventListener("pointerdown", onPointerDown, true);
    return () => document.removeEventListener("pointerdown", onPointerDown, true);
  }, []);
  const value = machineFacetValue(facet);
  const selected = facet.kind === "any" ? [] : Array.isArray(value) ? value : [value];
  const none = "\0unassigned";
  // Keep selections from shared/saved views removable even if the machine disappeared.
  const options = [...new Set([...machines, ...selected.filter((m) => m !== none)])].sort();
  const toggle = (machine: string) => onChange(parseMachineFacet(
    selected.includes(machine) ? selected.filter((m) => m !== machine) : [...selected, machine],
  ));
  return (
    <details
      ref={disclosure}
      className="machine-filter"
      onKeyDown={(event) => {
        if (event.key === "Escape" && event.currentTarget.open) {
          event.preventDefault();
          event.stopPropagation();
          event.currentTarget.open = false;
          summary.current?.focus();
        }
      }}
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) event.currentTarget.open = false;
      }}
    >
      <summary ref={summary} className="filter-select" aria-label="Machine filter">
        {selected.length === 0 ? "Any" : selected.length === 1
          ? selected[0] === none ? "None" : selected[0] : `${selected.length} selected`}
      </summary>
      <fieldset className="machine-filter-options">
        <legend>Match any selected machine</legend>
        <button type="button" className="btn btn-ghost btn-sm" onClick={() => onChange({ kind: "any" })}>Any machine</button>
        {[none, ...options].map((machine) => (
          <label key={machine}>
            <input type="checkbox" checked={selected.includes(machine)} onChange={() => toggle(machine)} />
            {machine === none ? "None (unassigned)" : machine}
          </label>
        ))}
      </fieldset>
    </details>
  );
}
