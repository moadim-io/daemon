import { useEffect } from "react";
import { Link, useNavigate } from "react-router-dom";
import { useAllRuns, useRoutines } from "../../api/hooks";
import { reltime } from "../../lib/cronUtils";
import { fmtUntil } from "../../lib/schedule";
import { applyTheme, loadThemeLight } from "../../lib/theme";
import { useNow } from "../../lib/useNow";
import { TILE_LABEL, TILE_STATES, buildTiles, tileCounts, type WallTile } from "./wallboardMath";

/** Data poll cadence — a wallboard is unattended, so it refreshes itself. */
const POLL_MS = 15_000;

/** Fleet runs fetched to derive tile state and history — the cap the Reliability page uses. */
const RUNS_LIMIT = 300;

function Tile({ tile, now }: { tile: WallTile; now: Date }) {
  const body = (
    <>
      <div className="wall-tile-hd">
        <span className="wall-tile-state">{TILE_LABEL[tile.state]}</span>
        <span className="wall-tile-history" aria-label={`last ${tile.history.length} runs`}>
          {tile.history.map((status, i) => (
            <span key={i} className={`wall-dot ${status}`} title={status} />
          ))}
        </span>
      </div>
      <div className="wall-tile-title">{tile.title}</div>
      <div className="wall-tile-reason">{tile.reason}</div>
      <div className="wall-tile-times">
        <span>last {tile.lastStartedAt === undefined ? "—" : reltime(tile.lastStartedAt)}</span>
        <span>next {tile.nextAt === undefined ? "—" : fmtUntil(now, tile.nextAt)}</span>
      </div>
    </>
  );
  const className = `wall-tile ${tile.state}`;
  return tile.latest ? (
    <Link className={className} to={`/runs/${tile.latest.routine_id}/${tile.latest.workbench}`}>
      {body}
    </Link>
  ) : (
    <div className={className}>{body}</div>
  );
}

/**
 * Kiosk / TV-mode status board at `/wall`, rendered outside the app shell: one big colour-coded
 * tile per routine (worst first), a count per state, a live clock, self-refreshing data.
 * `Esc` or EXIT returns to the Overview.
 */
export function WallboardPage() {
  const navigate = useNavigate();
  const routinesQuery = useRoutines({}, { refetchInterval: POLL_MS });
  const runsQuery = useAllRuns(RUNS_LIMIT, { refetchInterval: POLL_MS });
  const now = new Date(useNow(1_000));

  // The Shell normally applies the theme; this page lives outside it.
  useEffect(() => applyTheme(loadThemeLight()), []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") navigate("/");
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [navigate]);

  const tiles = buildTiles(routinesQuery.data ?? [], runsQuery.data ?? [], now);
  const counts = tileCounts(tiles);
  const updatedAtMs = Math.max(routinesQuery.dataUpdatedAt, runsQuery.dataUpdatedAt);
  const error = routinesQuery.error ?? runsQuery.error;

  return (
    <div className="wall">
      <header className="wall-hd">
        <span className="wall-brand">MOADIM · WALLBOARD</span>
        <div className="wall-counts">
          {TILE_STATES.map((state) => (
            <span key={state} className={`wall-count ${state}${counts[state] === 0 ? " zero" : ""}`}>
              <b>{counts[state]}</b> {TILE_LABEL[state]}
            </span>
          ))}
        </div>
        <span className="wall-clock">{now.toLocaleTimeString()}</span>
        <div className="wall-acts">
          <button
            type="button"
            className="btn"
            onClick={() => void document.documentElement.requestFullscreen?.().catch(() => undefined)}
          >
            FULLSCREEN
          </button>
          <Link className="btn" to="/">
            EXIT
          </Link>
        </div>
      </header>
      <div className="wall-meta">
        {error ? (
          <span className="wall-error">Data refresh failed: {error.message} — showing last known state</span>
        ) : (
          <span>
            updated {updatedAtMs === 0 ? "—" : new Date(updatedAtMs).toLocaleTimeString()} · refreshes every{" "}
            {POLL_MS / 1000}s
          </span>
        )}
      </div>
      {routinesQuery.isLoading ? (
        <p className="wall-empty">Loading…</p>
      ) : tiles.length === 0 ? (
        <p className="wall-empty">No enabled routines to show.</p>
      ) : (
        <div className="wall-grid">
          {tiles.map((tile) => (
            <Tile key={tile.id} tile={tile} now={now} />
          ))}
        </div>
      )}
    </div>
  );
}
