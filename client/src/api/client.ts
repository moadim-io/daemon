import createClient from "openapi-fetch";
import type { paths } from "./schema.gen";

/** Browser localStorage key holding the optional daemon API token for same-origin UI calls. */
export const API_TOKEN_STORAGE_KEY = "moadim.apiToken";

/** Server error body shape, matching `AppError`'s JSON response (src/error.rs). */
export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/** Read the operator-entered API token from browser storage, if available. */
export function storedApiToken(): string | undefined {
  try {
    return localStorage.getItem(API_TOKEN_STORAGE_KEY)?.trim() || undefined;
  } catch {
    return undefined;
  }
}

/** Persist the API token used by the served UI; pass a blank string to clear it. */
export function saveApiToken(token: string): void {
  try {
    const trimmed = token.trim();
    if (trimmed) localStorage.setItem(API_TOKEN_STORAGE_KEY, trimmed);
    else localStorage.removeItem(API_TOKEN_STORAGE_KEY);
  } catch {
    // Browser storage can be unavailable in private/sandboxed contexts; ignore token persistence.
  }
}

/** Same-origin fetch wrapper that attaches the optional UI token to `/api/v1` calls. */
export function authenticatedFetch(input: Parameters<typeof fetch>[0], init: Parameters<typeof fetch>[1] = {}) {
  const token = storedApiToken();
  if (!token) return fetch(input, init);
  const headers = new Headers(init.headers);
  if (!headers.has("authorization") && !headers.has("x-moadim-token")) {
    headers.set("authorization", `Bearer ${token}`);
  }
  return fetch(input, { ...init, headers });
}

/**
 * Same-origin relative base URL — the client is served by the same daemon
 * that serves `/api/v1`, so no host/port config is ever needed.
 */
export const api = createClient<paths>({ baseUrl: "/api/v1", fetch: authenticatedFetch });

function authHint(status: number): string {
  return status === 401
    ? " Set the UI token with localStorage.setItem('moadim.apiToken', '<MOADIM_API_TOKEN>')."
    : "";
}

/** Unwraps an openapi-fetch result, throwing {@link ApiError} on failure. */
export function unwrap<T>({
  data,
  error,
  response,
}: {
  data?: T;
  error?: { error?: string };
  response: Response;
}): T {
  if (error !== undefined || !response.ok) {
    throw new ApiError((error?.error ?? `HTTP ${response.status}`) + authHint(response.status), response.status);
  }
  if (data === undefined) {
    throw new ApiError("empty response body", response.status);
  }
  return data;
}

/** Like {@link unwrap} but for endpoints with no response body (e.g. a `204`). */
export function unwrapVoid({
  error,
  response,
}: {
  error?: { error?: string };
  response: Response;
}): void {
  if (error !== undefined || !response.ok) {
    throw new ApiError((error?.error ?? `HTTP ${response.status}`) + authHint(response.status), response.status);
  }
}
