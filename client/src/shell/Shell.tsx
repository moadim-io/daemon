import { useEffect, useState } from "react";
import { Outlet } from "react-router-dom";
import { useQueryClient } from "@tanstack/react-query";
import { useHealth, useMachine, useRenameMachine, useShutdown } from "../api/hooks";
import { applyTheme, loadThemeLight, saveThemeLight } from "../lib/theme";
import { Header } from "./Header";
import { Nav } from "./Nav";
import { CommandPalette } from "./CommandPalette";
import { RenameMachineDialog } from "./RenameMachineDialog";
import { ShutdownDialog } from "./ShutdownDialog";
import { ToastStack } from "./ToastStack";
import { useToasts } from "./toasts";

/** Persistent chrome around every routed page: header, nav, global dialogs, toasts. */
export function Shell() {
  const [lightTheme, setLightTheme] = useState(loadThemeLight);
  const [showShutdown, setShowShutdown] = useState(false);
  const [showPalette, setShowPalette] = useState(false);
  const [showRenameMachine, setShowRenameMachine] = useState(false);

  const queryClient = useQueryClient();
  const { addToast } = useToasts();
  const health = useHealth(30_000);
  const machine = useMachine();
  const shutdown = useShutdown();
  const renameMachine = useRenameMachine();

  useEffect(() => {
    applyTheme(lightTheme);
  }, [lightTheme]);

  // Global ⌘K / Ctrl-K toggles the palette; Escape dismisses whichever shell-level dialog is open.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setShowPalette((open) => !open);
      } else if (event.key === "Escape") {
        setShowShutdown(false);
        setShowRenameMachine(false);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  const toggleTheme = () => {
    setLightTheme((prev) => {
      const next = !prev;
      saveThemeLight(next);
      return next;
    });
  };

  const confirmShutdown = () => {
    setShowShutdown(false);
    shutdown.mutate(undefined, {
      onSuccess: () => {
        addToast("Server stopping…", "ok");
        void queryClient.invalidateQueries({ queryKey: ["health"] });
      },
      onError: (err) => addToast(`Stop failed: ${err.message}`, "err"),
    });
  };

  const confirmRenameMachine = async (name: string) => {
    try {
      await renameMachine.mutateAsync(name);
      setShowRenameMachine(false);
      addToast(`Machine renamed to "${name}"`, "ok");
    } catch (err) {
      addToast(`Rename failed: ${err instanceof Error ? err.message : String(err)}`, "err");
      throw err;
    }
  };

  return (
    <div className="app-shell">
      <Header
        health={health.data}
        healthOk={health.data?.running ?? false}
        light={lightTheme}
        machineName={machine.data?.name}
        onRefresh={() => void health.refetch()}
        onStop={() => setShowShutdown(true)}
        onPalette={() => setShowPalette((open) => !open)}
        onTheme={toggleTheme}
        onRenameMachine={() => setShowRenameMachine(true)}
      />
      <Nav />
      <div className="page">
        <Outlet />
      </div>
      <CommandPalette
        open={showPalette}
        onClose={() => setShowPalette(false)}
        onRefresh={() => void health.refetch()}
        onStop={() => setShowShutdown(true)}
        onToggleTheme={toggleTheme}
      />
      {showRenameMachine && (
        <RenameMachineDialog
          current={machine.data?.name ?? ""}
          onCancel={() => setShowRenameMachine(false)}
          onConfirm={confirmRenameMachine}
        />
      )}
      {showShutdown && (
        <ShutdownDialog onCancel={() => setShowShutdown(false)} onConfirm={confirmShutdown} />
      )}
      <ToastStack />
    </div>
  );
}
