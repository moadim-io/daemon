import { AppRoutes } from "./routes";
import { ToastProvider } from "./shell/toasts";

export function App() {
  return (
    <ToastProvider>
      <AppRoutes />
    </ToastProvider>
  );
}
