import { useEffect, useRef, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { api, unwrap } from "../api/client";
import { useTriggerRoutine } from "../api/hooks";
import { useToasts } from "./toasts";
import {
  badgeFor,
  buildCommands,
  clampSelection,
  lastIndex,
  nextIndex,
  prevIndex,
  rank,
  routeFor,
} from "./commandPaletteMatch";

const ROUTE_PATH: Record<string, string> = {
  home: "/",
  routines: "/routines",
  heatmap: "/heatmap",
  reliability: "/reliability",
  machines: "/machines",
  settings: "/settings",
};

export interface CommandPaletteProps {
  open: boolean;
  onClose: () => void;
  onRefresh: () => void;
  onStop: () => void;
  onToggleTheme: () => void;
}

export function CommandPalette({ open, onClose, onRefresh, onStop, onToggleTheme }: CommandPaletteProps) {
  const navigate = useNavigate();
  const triggerRoutine = useTriggerRoutine();
  const { addToast } = useToasts();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  // Re-fetch routines each time the palette opens.
  const { data: routines } = useQuery({
    queryKey: ["command-palette", "routines"],
    queryFn: async () => unwrap(await api.GET("/routines")),
    enabled: open,
  });

  // Reset query/selection during render (guarded on `open` actually changing)
  // rather than in an effect — see the `seededFrom` comment in SettingsPage.tsx
  // for the pattern. Focusing the input is a genuine side effect on an external
  // system (the DOM), so it stays in its own effect below.
  const [prevOpen, setPrevOpen] = useState(open);
  if (open !== prevOpen) {
    setPrevOpen(open);
    if (open) {
      setQuery("");
      setSelected(0);
    }
  }

  useEffect(() => {
    if (open) inputRef.current?.focus();
  }, [open]);

  if (!open) return null;

  const commands = buildCommands(routines ?? []);
  const order = rank(commands, query);
  const sel = clampSelection(selected, order.length);

  // A routine result opens straight into its history — the same `?history=<id>`
  // deep link the Overview page's recent-runs panel uses — rather than the bare
  // routines list, so picking a specific routine actually lands on it.
  const launch = (row: number) => {
    const command = commands[order[row] ?? -1];
    if (command) {
      switch (command.kind) {
        case "action-refresh":
          onRefresh();
          break;
        case "action-stop":
          onStop();
          break;
        case "action-toggle-theme":
          onToggleTheme();
          break;
        case "routine":
          navigate(`/routines?history=${encodeURIComponent(command.routineId ?? "")}`);
          break;
        default: {
          const routeKind = routeFor(command.kind);
          if (routeKind) navigate(ROUTE_PATH[routeKind] ?? "/");
        }
      }
    }
    onClose();
  };

  // Quick action: fire a routine directly from its result row without navigating
  // away, mirroring the modifier-key secondary-action pattern of Linear/Raycast's
  // command palettes.
  const triggerFromRow = (row: number) => {
    const command = commands[order[row] ?? -1];
    if (!command || command.kind !== "routine" || !command.routineId) return;
    const id = command.routineId;
    const title = command.title;
    triggerRoutine.mutate(id, {
      onSuccess: () => addToast(`Triggered "${title}"`, "ok"),
      onError: (e) => addToast(`Trigger failed: ${e.message}`, "err"),
    });
    onClose();
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        setSelected(nextIndex(sel, order.length));
        break;
      case "ArrowUp":
        e.preventDefault();
        setSelected(prevIndex(sel));
        break;
      case "Home":
        e.preventDefault();
        setSelected(0);
        break;
      case "End":
        e.preventDefault();
        setSelected(lastIndex(order.length));
        break;
      case "Enter":
        e.preventDefault();
        if (e.metaKey || e.ctrlKey) {
          triggerFromRow(sel);
        } else {
          launch(sel);
        }
        break;
      case "Escape":
        e.preventDefault();
        onClose();
        break;
    }
  };

  const activeId = order.length > 0 ? `cmdk-opt-${sel}` : undefined;

  return (
    <div className="overlay" onClick={onClose}>
      <div
        className="cmdk"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="cmdk-search">
          <span aria-hidden="true">›</span>
          <input
            ref={inputRef}
            className="cmdk-input"
            type="text"
            placeholder="Search pages, routines…"
            autoComplete="off"
            spellCheck={false}
            role="combobox"
            aria-expanded="true"
            aria-controls="cmdk-listbox"
            aria-autocomplete="list"
            aria-activedescendant={activeId}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setSelected(0);
            }}
            onKeyDown={onKeyDown}
          />
          <span className="cmdk-hint" aria-hidden="true">
            ESC
          </span>
        </div>
        {order.length === 0 ? (
          <div className="cmdk-empty">
            <div>NO MATCHES</div>
            <div>no page or routine matches</div>
          </div>
        ) : (
          <ul id="cmdk-listbox" className="cmdk-list" role="listbox" aria-label="Results">
            {order.map((cmdIdx, row) => {
              const command = commands[cmdIdx];
              if (!command) return null;
              const active = row === sel;
              return (
                <li
                  key={cmdIdx}
                  id={`cmdk-opt-${row}`}
                  className={`cmdk-row${active ? " active" : ""}`}
                  role="option"
                  aria-selected={active}
                  onClick={() => launch(row)}
                  onMouseEnter={() => setSelected(row)}
                >
                  <span className="kind-badge">{badgeFor(command.kind)}</span>
                  <span className="cmdk-row-text">
                    <span className="cmdk-row-title">{command.title}</span>
                    <span className="cmdk-row-sub">{command.subtitle}</span>
                  </span>
                  {command.kind === "routine" && (
                    <button
                      type="button"
                      className="btn btn-ghost btn-sm cmdk-row-action"
                      title="Trigger now (⌘⏎)"
                      aria-label={`Trigger ${command.title} now`}
                      onClick={(e) => {
                        e.stopPropagation();
                        triggerFromRow(row);
                      }}
                    >
                      ⚡
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        )}
        <div className="cmdk-foot" aria-hidden="true">
          <span>
            <span className="cmdk-key">↑↓</span> navigate
          </span>
          <span>
            <span className="cmdk-key">↵</span> open
          </span>
          <span>
            <span className="cmdk-key">⌘↵</span> trigger routine
          </span>
          <span>
            <span className="cmdk-key">esc</span> close
          </span>
        </div>
      </div>
    </div>
  );
}
