import { describe, expect, it } from "vitest";
import { ApiError, unwrap, unwrapVoid } from "./client";

function resp(status: number): Response {
  return new Response(null, { status });
}

describe("unwrap", () => {
  it("returns data on success", () => {
    expect(unwrap({ data: { ok: true }, response: resp(200) })).toEqual({ ok: true });
  });

  it("throws ApiError with server message on error body", () => {
    expect(() => unwrap({ error: { error: "boom" }, response: resp(400) })).toThrow(ApiError);
    try {
      unwrap({ error: { error: "boom" }, response: resp(400) });
    } catch (e) {
      expect((e as ApiError).message).toBe("boom");
      expect((e as ApiError).status).toBe(400);
    }
  });

  it("throws a generic message when the error body has no message", () => {
    expect(() => unwrap({ error: {}, response: resp(500) })).toThrow("HTTP 500");
  });

  it("throws when data is missing despite a 2xx response", () => {
    expect(() => unwrap({ response: resp(200) })).toThrow("empty response body");
  });
});

describe("unwrapVoid", () => {
  it("does not throw on success", () => {
    expect(() => unwrapVoid({ response: resp(204) })).not.toThrow();
  });

  it("throws ApiError on failure", () => {
    expect(() => unwrapVoid({ error: { error: "nope" }, response: resp(404) })).toThrow("nope");
  });

  it("throws a generic message when the error body has no message", () => {
    expect(() => unwrapVoid({ response: resp(500) })).toThrow("HTTP 500");
  });
});
