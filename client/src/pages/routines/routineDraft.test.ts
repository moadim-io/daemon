// clone_title/TTL_PRESETS cases ported 1:1 from ui/src/routines/form_tests.rs; the remaining
// cases cover the round-trip text<->structured-field helpers form_tests.rs didn't exercise.
import { describe, expect, it } from "vitest";
import { cloneTitle, parseTtl, reposToText, tagsToText, textToRepos, textToTags } from "./routineDraft";
import { TTL_PRESETS, formatTtl } from "./ttl";

describe("routineDraft — cloneTitle", () => {
  it("prepends Copy of", () => {
    expect(cloneTitle("Daily report")).toBe("Copy of Daily report");
  });

  it("does not double-prefix", () => {
    expect(cloneTitle("Copy of Daily report")).toBe("Copy of Daily report");
  });

  it("preserves empty string", () => {
    expect(cloneTitle("")).toBe("Copy of ");
  });
});

describe("ttl — TTL_PRESETS", () => {
  it("maps labels to seconds", () => {
    expect(TTL_PRESETS).toEqual([
      ["3600", "1h"],
      ["86400", "1d"],
      ["604800", "7d"],
      ["2592000", "30d"],
    ]);
  });
});

describe("ttl — formatTtl", () => {
  it("null/undefined means default", () => {
    expect(formatTtl(null)).toBe("default");
    expect(formatTtl(undefined)).toBe("default");
  });
  it("picks the largest evenly-dividing unit", () => {
    expect(formatTtl(0)).toBe("0s");
    expect(formatTtl(45)).toBe("45s");
    expect(formatTtl(120)).toBe("2m");
    expect(formatTtl(7_200)).toBe("2h");
    expect(formatTtl(172_800)).toBe("2d");
  });
});

describe("routineDraft — repositories text <-> structured", () => {
  it("round-trips a repo with a branch", () => {
    const repos = [{ repository: "https://github.com/org/repo", branch: "main" }];
    const text = reposToText(repos);
    expect(text).toBe("https://github.com/org/repo main");
    expect(textToRepos(text)).toEqual(repos);
  });

  it("round-trips a repo without a branch", () => {
    const repos = [{ repository: "https://github.com/org/repo", branch: null }];
    expect(reposToText(repos)).toBe("https://github.com/org/repo");
    expect(textToRepos("https://github.com/org/repo")).toEqual(repos);
  });

  it("skips blank lines", () => {
    expect(textToRepos("a\n\nb")).toEqual([
      { repository: "a", branch: null },
      { repository: "b", branch: null },
    ]);
  });
});

describe("routineDraft — tags text <-> structured", () => {
  it("round-trips comma-separated tags", () => {
    expect(tagsToText(["nightly", "prod"])).toBe("nightly, prod");
    expect(textToTags("nightly, prod")).toEqual(["nightly", "prod"]);
  });

  it("trims and drops blanks", () => {
    expect(textToTags(" a ,, b ,")).toEqual(["a", "b"]);
  });
});

describe("routineDraft — parseTtl", () => {
  it("blank means server default", () => {
    expect(parseTtl("")).toBeNull();
    expect(parseTtl("   ")).toBeNull();
  });

  it("parses a valid integer", () => {
    expect(parseTtl("604800")).toBe(604_800);
  });

  it("non-numeric input falls back to default", () => {
    expect(parseTtl("abc")).toBeNull();
    expect(parseTtl("12.5")).toBeNull();
    expect(parseTtl("-5")).toBeNull();
  });
});
