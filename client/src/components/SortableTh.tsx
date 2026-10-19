import type { SortDir, SortState } from "../lib/tableSort";

/**
 * A sortable column header: a `<th>` carrying `aria-sort` that wraps a native button, so the
 * column is reachable with Tab and toggled with Enter/Space (WAI-ARIA APG sortable table).
 */
export function SortableTh<K extends string>({
  label,
  col,
  sort,
  defaultDir,
  onSort,
}: {
  label: string;
  col: K;
  sort: SortState<K> | null;
  defaultDir: SortDir;
  onSort: (col: K, defaultDir: SortDir) => void;
}) {
  const dir = sort?.key === col ? sort.dir : null;
  return (
    <th
      className={dir === null ? "th-sort" : "th-sort th-sort-active"}
      aria-sort={dir === null ? "none" : dir === "asc" ? "ascending" : "descending"}
    >
      <button type="button" className="th-sort-btn" onClick={() => onSort(col, defaultDir)}>
        {label}
        {dir === null ? "" : dir === "asc" ? " ▲" : " ▼"}
      </button>
    </th>
  );
}
