import { useMemo, useState, type CSSProperties } from "react";
import type { RoutineResponse } from "../../api/hooks";
import { scheduleList } from "../../lib/schedule";
import { healthBadge, healthBadgeClass, healthTooltip, triggerButtonTitle } from "./filter";
import { routineHealth } from "./filter";
import { NextRunCell } from "./RoutineRow";

interface TreeRoutineActions {
  onEdit: (id: string) => void;
  onClone: (id: string) => void;
  onDelete: (id: string, title: string) => void;
  onMove: (id: string) => void;
  onToggle: (id: string, enabled: boolean) => void;
  onTrigger: (id: string) => void;
  onLogs: (id: string) => void;
  onHistory: (id: string) => void;
  onFlags: (id: string) => void;
}

export interface RoutineFilesystemTreeProps extends TreeRoutineActions {
  routines: RoutineResponse[];
  loading: boolean;
  filterActive: boolean;
  now: Date;
  onClearFilters: () => void;
}

interface FolderNode {
  name: string;
  path: string;
  folders: Map<string, FolderNode>;
  routines: RoutineResponse[];
}

function routinePath(routine: RoutineResponse): string {
  const slug = routine.slug?.trim() || routine.title.trim() || routine.id;
  const folder = routine.folder?.trim();
  return folder ? `${folder}/${slug}` : slug;
}

function buildTree(routines: readonly RoutineResponse[]): FolderNode {
  const root: FolderNode = { name: "", path: "", folders: new Map(), routines: [] };
  for (const routine of routines) {
    const parts = (routine.folder ?? "").split("/").map((x) => x.trim()).filter(Boolean);
    let node = root;
    for (const part of parts) {
      const path = node.path === "" ? part : `${node.path}/${part}`;
      let child = node.folders.get(part);
      if (child === undefined) {
        child = { name: part, path, folders: new Map(), routines: [] };
        node.folders.set(part, child);
      }
      node = child;
    }
    node.routines.push(routine);
  }
  sortNode(root);
  return root;
}

function sortNode(node: FolderNode) {
  node.routines.sort((a, b) => routinePath(a).localeCompare(routinePath(b)) || a.id.localeCompare(b.id));
  for (const child of node.folders.values()) sortNode(child);
}

function childFolders(node: FolderNode): FolderNode[] {
  return [...node.folders.values()].sort((a, b) => a.name.localeCompare(b.name));
}

function depthStyle(depth: number): CSSProperties {
  return { "--depth": depth } as CSSProperties;
}

export function RoutineFilesystemTree({ routines, loading, filterActive, now, onClearFilters, ...actions }: RoutineFilesystemTreeProps) {
  const root = useMemo(() => buildTree(routines), [routines]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  const toggleFolder = (path: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (!next.delete(path)) next.add(path);
      return next;
    });
  };

  if (loading) {
    return (
      <div className="routine-fs-wrap">
        <div className="empty"><div className="spinner" /></div>
      </div>
    );
  }

  if (routines.length === 0) {
    return (
      <div className="routine-fs-wrap">
        <div className="empty">
          <div className="empty-icon">{filterActive ? "⊘" : "⧗"}</div>
          <div className="empty-msg">{filterActive ? "NO ROUTINES MATCH" : "NO ROUTINES SCHEDULED"}</div>
          <div className="empty-sub">
            {filterActive ? (
              <button type="button" className="btn btn-ghost btn-sm" onClick={onClearFilters}>CLEAR FILTERS</button>
            ) : (
              "press + NEW ROUTINE to create one"
            )}
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="routine-fs-wrap">
      <div className="routine-fs-summary">{routines.length} routine(s) shown by filesystem location</div>
      <div className="routine-fs-tree" role="tree" aria-label="Routine filesystem">
        {root.routines.map((routine) => <RoutineFile key={routine.id} routine={routine} now={now} depth={0} {...actions} />)}
        {childFolders(root).map((folder) => (
          <FolderBranch key={folder.path} node={folder} now={now} depth={0} collapsed={collapsed} onToggle={toggleFolder} actions={actions} />
        ))}
      </div>
    </div>
  );
}

function FolderBranch({ node, now, depth, collapsed, onToggle, actions }: { node: FolderNode; now: Date; depth: number; collapsed: Set<string>; onToggle: (path: string) => void; actions: TreeRoutineActions }) {
  const isCollapsed = collapsed.has(node.path);
  const count = countRoutines(node);
  return (
    <div className="routine-fs-branch">
      <div className="routine-fs-folder" role="treeitem" aria-expanded={!isCollapsed} aria-label={`${node.name} folder, ${count} routine${count === 1 ? "" : "s"}`} style={depthStyle(depth)}>
        <button type="button" className="routine-fs-folder-toggle" aria-label={`${isCollapsed ? "Expand" : "Collapse"} ${node.name} folder`} onClick={() => onToggle(node.path)}>
          <span className="routine-fs-caret">{isCollapsed ? "▸" : "▾"}</span>
          <span className="routine-fs-icon">📁</span>
          <span className="routine-fs-folder-name">{node.name}</span>
          <span className="routine-fs-count">{count}</span>
        </button>
      </div>
      {!isCollapsed && (
        <div role="group">
          {node.routines.map((routine) => <RoutineFile key={routine.id} routine={routine} now={now} depth={depth + 1} {...actions} />)}
          {childFolders(node).map((folder) => <FolderBranch key={folder.path} node={folder} now={now} depth={depth + 1} collapsed={collapsed} onToggle={onToggle} actions={actions} />)}
        </div>
      )}
    </div>
  );
}

function countRoutines(node: FolderNode): number {
  let total = node.routines.length;
  for (const child of node.folders.values()) total += countRoutines(child);
  return total;
}

function RoutineFile({ routine, now, depth, onEdit, onClone, onDelete, onMove, onToggle, onTrigger, onLogs, onHistory, onFlags }: { routine: RoutineResponse; now: Date; depth: number } & TreeRoutineActions) {
  const path = routinePath(routine);
  const health = routineHealth(routine, now);
  const schedules = scheduleList(routine);
  return (
    <div className="routine-fs-file" role="treeitem" aria-label={`${path} routine`} style={depthStyle(depth)}>
      <div className="routine-fs-file-main">
        <span className="routine-fs-icon">📄</span>
        <div className="routine-fs-file-copy">
          <div className="routine-fs-file-title">{routine.title}</div>
          <div className="routine-fs-file-path" title={routine.rel_path}>{path}</div>
        </div>
      </div>
      <div className="routine-fs-meta"><span>{schedules.join(" · ")}</span><NextRunCell routine={routine} now={now} /></div>
      <div className="routine-fs-status"><span className={healthBadgeClass(health)} title={healthTooltip(routine, health)}>{healthBadge(health)}</span><label className="toggle"><input type="checkbox" checked={routine.enabled} onChange={(e) => onToggle(routine.id, e.target.checked)} /><div className="toggle-track" /></label></div>
      <div className="routine-fs-actions">
        <button type="button" className="act-btn run" aria-label="Run now" title={triggerButtonTitle(routine)} disabled={!routine.enabled || routine.power_saving} onClick={() => onTrigger(routine.id)}>RUN</button>
        <button type="button" className="act-btn" onClick={() => onLogs(routine.id)}>Logs</button>
        <button type="button" className="act-btn" onClick={() => onHistory(routine.id)}>History</button>
        <button type="button" className="act-btn" onClick={() => onFlags(routine.id)}>Flags</button>
        <button type="button" className="act-btn" onClick={() => onEdit(routine.id)}>Edit</button>
        <button type="button" className="act-btn" onClick={() => onMove(routine.id)}>Move folder</button>
        <button type="button" className="act-btn" onClick={() => onClone(routine.id)}>Clone</button>
        <button type="button" className="act-btn danger" onClick={() => onDelete(routine.id, routine.title)}>Delete</button>
      </div>
    </div>
  );
}
