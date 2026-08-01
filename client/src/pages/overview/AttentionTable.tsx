import { Link } from "react-router-dom";
import { ATTENTION_BADGE, ATTENTION_DETAIL, type AttentionItem } from "./overviewLogic";

/**
 * The NEEDS ATTENTION triage table: one row per enabled-but-broken entity,
 * worst fault first. Direct port of `ui/src/overview_attention.rs (removed)`'s
 * `AttentionTable`. Rendered only when `items` is non-empty (see the page).
 */
export function AttentionTable({ items }: { items: AttentionItem[] }) {
  return (
    <div className="table-wrap attn-wrap overview-table-wrap">
      <table className="overview-card-table">
        <thead>
          <tr>
            <th>TYPE</th>
            <th>NAME</th>
            <th>ISSUE</th>
            <th>DETAIL</th>
          </tr>
        </thead>
        <tbody>
          {items.map((item, i) => (
            <tr key={i}>
              <td data-label="Type">
                <span className="kind-badge routine">ROUTINE</span>
              </td>
              <td data-label="Name">
                <Link className="ov-name-link" to="/routines">
                  {item.label}
                </Link>
              </td>
              <td data-label="Issue">
                <span className="attn-badge">{ATTENTION_BADGE[item.reason]}</span>
              </td>
              <td className="attn-detail" data-label="Detail">
                {item.reason === "has-open-flags" && item.flagCount > 0
                  ? `${item.flagCount} open flag${item.flagCount === 1 ? "" : "s"} — needs review`
                  : ATTENTION_DETAIL[item.reason]}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
