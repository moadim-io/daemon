import { useMutation, useQuery, useQueryClient, type UseQueryOptions } from "@tanstack/react-query";
import { api, unwrap, unwrapVoid } from "./client";
import type { components } from "./schema.gen";

type Schemas = components["schemas"];
export type HealthResponse = Schemas["HealthResponse"];
export type RoutineResponse = Schemas["RoutineResponse"];
export type Routine = Schemas["Routine"];
export type CreateRoutineRequest = Schemas["CreateRoutineRequest"];
export type UpdateRoutineRequest = Schemas["UpdateRoutineRequest"];
export type Flag = Schemas["Flag"];
export type CreateFlagRequest = Schemas["CreateFlagRequest"];
export type RunSummary = Schemas["RunSummary"];
export type FleetRunSummary = Schemas["FleetRunSummary"];
export type LockStatus = Schemas["LockStatus"];

// ─── Health ─────────────────────────────────────────────────────────────────

/** Polls `GET /health`; `refetchInterval` keeps the shell's status live. */
export function useHealth(refetchIntervalMs = 10_000) {
  return useQuery({
    queryKey: ["health"],
    queryFn: async () => unwrap(await api.GET("/health")),
    refetchInterval: refetchIntervalMs,
    retry: false,
  });
}

export function useShutdown() {
  return useMutation({
    mutationFn: async () => unwrap(await api.POST("/shutdown")),
  });
}

export function useRestart() {
  return useMutation({
    mutationFn: async () => unwrap(await api.POST("/restart")),
  });
}

// ─── Machine ────────────────────────────────────────────────────────────────

export function useMachine() {
  return useQuery({
    queryKey: ["machine"],
    queryFn: async () => unwrap(await api.GET("/machine")),
  });
}

export function useMachines() {
  return useQuery({
    queryKey: ["machines"],
    queryFn: async () => unwrap(await api.GET("/machines")),
  });
}

export function useRenameMachine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (name: string) =>
      unwrap(await api.PUT("/machine", { body: { name } })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["machine"] });
      void queryClient.invalidateQueries({ queryKey: ["machines"] });
    },
  });
}

// ─── Agents ─────────────────────────────────────────────────────────────────

export function useAgents() {
  return useQuery({
    queryKey: ["agents"],
    queryFn: async () => unwrap(await api.GET("/agents")),
  });
}

// ─── User prompt config ────────────────────────────────────────────────────

export function useUserPrompt() {
  return useQuery({
    queryKey: ["config", "user-prompt"],
    queryFn: async () => {
      const { data, error, response } = await api.GET("/config/user-prompt", {
        parseAs: "text",
      });
      return unwrap<string>({ data: data as string | undefined, error, response });
    },
  });
}

export function useSetUserPrompt() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (content: string) =>
      unwrapVoid(await api.PUT("/config/user-prompt", { body: { content } })),
    onSuccess: () => {
      void queryClient.invalidateQueries({ queryKey: ["config", "user-prompt"] });
    },
  });
}

// ─── Routines ───────────────────────────────────────────────────────────────

export interface ListRoutinesParams {
  repository?: string;
  sort?: Schemas["RoutineSort"];
  order?: Schemas["SortOrder"];
  local_only?: boolean;
  include_prompts?: boolean;
}

export function useRoutines(
  params: ListRoutinesParams = {},
  options?: Partial<UseQueryOptions<RoutineResponse[]>>,
) {
  return useQuery({
    queryKey: ["routines", params],
    queryFn: async () => unwrap(await api.GET("/routines", { params: { query: params } })),
    ...options,
  });
}

export function useRoutine(id: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id],
    queryFn: async () => unwrap(await api.GET("/routines/{id}", { params: { path: { id } } })),
    enabled: enabled && id.length > 0,
  });
}

function invalidateRoutines(queryClient: ReturnType<typeof useQueryClient>, id?: string) {
  void queryClient.invalidateQueries({ queryKey: ["routines"] });
  if (id) void queryClient.invalidateQueries({ queryKey: ["routines", id] });
}

export function useCreateRoutine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (body: CreateRoutineRequest) =>
      unwrap(await api.POST("/routines", { body })),
    onSuccess: () => invalidateRoutines(queryClient),
  });
}

/** `PATCH /routines/{id}` — partial-merge update. */
export function useUpdateRoutine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, body }: { id: string; body: UpdateRoutineRequest }) =>
      unwrap(await api.PATCH("/routines/{id}", { params: { path: { id } }, body })),
    onSuccess: (_data, { id }) => invalidateRoutines(queryClient, id),
  });
}

export function useDeleteRoutine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(await api.DELETE("/routines/{id}", { params: { path: { id } } })),
    onSuccess: (_data, id) => invalidateRoutines(queryClient, id),
  });
}

export function useTriggerRoutine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(await api.POST("/routines/{id}/trigger", { params: { path: { id } } })),
    onSuccess: (_data, id) => invalidateRoutines(queryClient, id),
  });
}

export function useScheduledTriggerRoutine() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(await api.POST("/routines/{id}/scheduled-trigger", { params: { path: { id } } })),
    onSuccess: (_data, id) => invalidateRoutines(queryClient, id),
  });
}

export function useCleanupRoutines() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async () => unwrap(await api.POST("/routines/cleanup")),
    onSuccess: () => invalidateRoutines(queryClient),
  });
}

// ─── Global lock ────────────────────────────────────────────────────────────

export function useLockStatus() {
  return useQuery({
    queryKey: ["routines", "lock"],
    queryFn: async () => unwrap(await api.GET("/routines/lock")),
  });
}

export function useLock() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (scope: Schemas["LockRequest"]["scope"]) =>
      unwrap(await api.POST("/routines/lock", { body: { scope } })),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["routines", "lock"] }),
  });
}

export function useUnlock() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (scope: string) =>
      unwrap(await api.DELETE("/routines/lock", { params: { query: { scope } } })),
    onSuccess: () => void queryClient.invalidateQueries({ queryKey: ["routines", "lock"] }),
  });
}

// ─── Runs ───────────────────────────────────────────────────────────────────

/** `GET /routines/runs` — most recent runs across the whole fleet. */
export function useAllRuns(limit?: number, options?: Partial<UseQueryOptions<FleetRunSummary[]>>) {
  return useQuery({
    queryKey: ["routines", "runs", limit],
    queryFn: async () =>
      unwrap(await api.GET("/routines/runs", { params: { query: { limit } } })),
    ...options,
  });
}

export function useRoutineRuns(id: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "runs"],
    queryFn: async () => unwrap(await api.GET("/routines/{id}/runs", { params: { path: { id } } })),
    enabled: enabled && id.length > 0,
  });
}

async function fetchText(
  result: Promise<{ data?: unknown; error?: { error?: string }; response: Response }>,
): Promise<string> {
  const { data, error, response } = await result;
  return unwrap<string>({ data: data as string | undefined, error, response });
}

export function useRoutineLogs(id: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "logs"],
    queryFn: () => fetchText(api.GET("/routines/{id}/logs", { params: { path: { id } }, parseAs: "text" })),
    enabled: enabled && id.length > 0,
  });
}

export function usePromptPreview(id: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "prompt-preview"],
    queryFn: () =>
      fetchText(api.GET("/routines/{id}/prompt-preview", { params: { path: { id } }, parseAs: "text" })),
    enabled: enabled && id.length > 0,
  });
}

export function useRunLog(id: string, workbench: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "runs", workbench, "log"],
    queryFn: () =>
      fetchText(
        api.GET("/routines/{id}/runs/{workbench}/log", {
          params: { path: { id, workbench } },
          parseAs: "text",
        }),
      ),
    enabled: enabled && id.length > 0 && workbench.length > 0,
  });
}

export function useRunSummary(id: string, workbench: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "runs", workbench, "summary"],
    queryFn: () =>
      fetchText(
        api.GET("/routines/{id}/runs/{workbench}/summary", {
          params: { path: { id, workbench } },
          parseAs: "text",
        }),
      ),
    enabled: enabled && id.length > 0 && workbench.length > 0,
  });
}

// ─── Flags ──────────────────────────────────────────────────────────────────

export function useFlags(id: string, enabled = true) {
  return useQuery({
    queryKey: ["routines", id, "flags"],
    queryFn: async () => unwrap(await api.GET("/routines/{id}/flags", { params: { path: { id } } })),
    enabled: enabled && id.length > 0,
  });
}

export function useCreateFlag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, body }: { id: string; body: CreateFlagRequest }) =>
      unwrap(await api.POST("/routines/{id}/flags", { params: { path: { id } }, body })),
    onSuccess: (_data, { id }) =>
      void queryClient.invalidateQueries({ queryKey: ["routines", id, "flags"] }),
  });
}

export function useResolveFlag() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, filename }: { id: string; filename: string }) =>
      unwrapVoid(
        await api.DELETE("/routines/{id}/flags/{filename}", {
          params: { path: { id, filename } },
        }),
      ),
    onSuccess: (_data, { id }) =>
      void queryClient.invalidateQueries({ queryKey: ["routines", id, "flags"] }),
  });
}

// ─── iCal feed ──────────────────────────────────────────────────────────────

/** Direct link (not fetched — meant for "subscribe" `<a href>`s), optionally scoped to one routine. */
export function icalFeedUrl(routineId?: string): string {
  return routineId
    ? `/api/v1/routines.ics?routine=${encodeURIComponent(routineId)}`
    : "/api/v1/routines.ics";
}
