import { Navigate, Route, Routes } from "react-router-dom";
import { Shell } from "./shell/Shell";
import { OverviewPage } from "./pages/overview/OverviewPage";
import { RoutinesPage } from "./pages/routines/RoutinesPage";
import { HeatmapPage } from "./pages/heatmap/HeatmapPage";
import { ReliabilityPage } from "./pages/reliability/ReliabilityPage";
import { MachinesPage } from "./pages/machines/MachinesPage";
import { SettingsPage } from "./pages/settings/SettingsPage";

/**
 * Route paths are resolved from the server root — the daemon serves this SPA at `GET /`:
 * `/`, `/routines`, `/heatmap`, `/reliability`, `/machines`, `/settings` — an unknown path
 * redirects home.
 */
export function AppRoutes() {
  return (
    <Routes>
      <Route element={<Shell />}>
        <Route index element={<OverviewPage />} />
        <Route path="routines" element={<RoutinesPage />} />
        <Route path="heatmap" element={<HeatmapPage />} />
        <Route path="reliability" element={<ReliabilityPage />} />
        <Route path="machines" element={<MachinesPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  );
}
