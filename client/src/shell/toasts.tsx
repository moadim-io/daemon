import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from "react";

export type ToastKind = "ok" | "err";

export interface Toast {
  id: number;
  msg: string;
  kind: ToastKind;
}

interface ToastContextValue {
  toasts: Toast[];
  addToast: (msg: string, kind: ToastKind) => void;
}

const ToastContext = createContext<ToastContextValue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(0);

  const addToast = useCallback((msg: string, kind: ToastKind) => {
    const id = nextId.current++;
    setToasts((prev) => [...prev, { id, msg, kind }]);
    setTimeout(() => setToasts((prev) => prev.filter((t) => t.id !== id)), 5_000);
  }, []);

  return <ToastContext.Provider value={{ toasts, addToast }}>{children}</ToastContext.Provider>;
}

export function useToasts(): ToastContextValue {
  const ctx = useContext(ToastContext);
  if (!ctx) throw new Error("useToasts must be used within a ToastProvider");
  return ctx;
}
