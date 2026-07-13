import createClient from "openapi-fetch";
import type { paths } from "./schema.gen";

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

/**
 * Same-origin relative base URL — the client is served by the same daemon
 * that serves `/api/v1`, so no host/port config is ever needed.
 */
export const api = createClient<paths>({ baseUrl: "/api/v1" });

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
    throw new ApiError(error?.error ?? `HTTP ${response.status}`, response.status);
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
    throw new ApiError(error?.error ?? `HTTP ${response.status}`, response.status);
  }
}
