import { useState } from "react";
import { useMachines } from "../../api/hooks";

export interface MachinesPickerProps {
  value: string[];
  onChange: (machines: string[]) => void;
}

/** Chip-based machine-targeting picker backed by `GET /api/v1/machines`. */
export function MachinesPicker({ value, onChange }: MachinesPickerProps) {
  const machines = useMachines();
  const [newName, setNewName] = useState("");

  const known = machines.data ?? [];
  const candidates = [...new Set([...known, ...value])].sort();

  const toggle = (name: string) => {
    if (value.includes(name)) onChange(value.filter((m) => m !== name));
    else onChange([...value, name].sort());
  };

  const addNew = () => {
    const name = newName.trim();
    if (name !== "" && !value.includes(name)) onChange([...value, name].sort());
    setNewName("");
  };

  return (
    <div className="form-group">
      <label className="form-label">
        MACHINES <span style={{ color: "var(--text-faint)" }}>(pick targets; none = runs nowhere)</span>
      </label>
      {candidates.length === 0 ? (
        <div className="machine-empty">No machines known yet — add one below.</div>
      ) : (
        <div className="machine-chips">
          {candidates.map((name) => (
            <button
              type="button"
              key={name}
              className={value.includes(name) ? "machine-chip on" : "machine-chip"}
              onClick={() => toggle(name)}
            >
              {name}
            </button>
          ))}
        </div>
      )}
      <div className="machine-add">
        <input
          type="text"
          className="form-input"
          placeholder="add a machine name"
          autoComplete="off"
          spellCheck={false}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <button type="button" className="btn btn-ghost btn-sm" onClick={addNew}>
          ADD
        </button>
      </div>
    </div>
  );
}
