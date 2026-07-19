import { Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "./shell/Shell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { RoutinesPage } from "./pages/routines/RoutinesPage";
import { HeatmapPage } from "./pages/heatmap/HeatmapPage";
import { SettingsPage } from "./pages/settings/SettingsPage";

/**
 * Route paths are resolved relative to `<BrowserRouter basename="/client">`
 * (see main.tsx) — 1:1 with `ui/src/main.rs`'s `Route` enum (`/`, `/routines`,
 * `/heatmap`, `/settings`), an unknown path redirects home.
 */
export function AppRoutes() {
  return (
    <Routes>
      <Route element={<Shell />}>
        <Route index element={<OverviewPage />} />
        <Route path="routines" element={<RoutinesPage />} />
        <Route path="heatmap" element={<HeatmapPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
