// Ported 1:1 from ui/src/log_viewer_tests.rs.
import { describe, expect, it } from "vitest";
import { filterLines, highlightSegments, matchCount } from "./logSearch";

describe("logViewer — filterLines", () => {
  it("empty query returns all lines numbered from one", () => {
    expect(filterLines("alpha\nbeta\ngamma", "")).toEqual([
      [1, "alpha"],
      [2, "beta"],
      [3, "gamma"],
    ]);
  });

  it("blank query whitespace returns all", () => {
    expect(filterLines("x\ny", "   ").length).toBe(2);
  });

  it("query filters case insensitively", () => {
    const content = "INFO: server started\nERROR: connection refused\nWARN: retrying";
    const out = filterLines(content, "ERROR");
    expect(out.length).toBe(1);
    expect(out[0]).toEqual([2, "ERROR: connection refused"]);
  });

  it("query preserves original line numbers", () => {
    expect(filterLines("line 1\nline 2\nline 3\nline 4", "line 3")).toEqual([[3, "line 3"]]);
  });

  it("no match returns empty array", () => {
    expect(filterLines("hello world", "zzz")).toEqual([]);
  });

  it("single line exact match", () => {
    expect(filterLines("hello", "hello")).toEqual([[1, "hello"]]);
  });

  it("single line no match", () => {
    expect(filterLines("hello", "world")).toEqual([]);
  });

  it("trailing newline does not produce extra numbered line", () => {
    expect(filterLines("alpha\n", "").length).toBe(1);
  });
});

describe("logViewer — matchCount", () => {
  it("blank query count equals total", () => {
    expect(matchCount("a\nb\nc", "")).toEqual([3, 3]);
  });

  it("whitespace query treated as blank", () => {
    const [hits, total] = matchCount("a\nb", "   ");
    expect(hits).toBe(total);
  });

  it("partial match count", () => {
    expect(matchCount("foo\nbar\nfoo bar", "foo")).toEqual([2, 3]);
  });

  it("no match count is zero", () => {
    expect(matchCount("a\nb", "z")).toEqual([0, 2]);
  });

  it("match count is case insensitive", () => {
    const [hits] = matchCount("ERROR\nerror\nOk", "error");
    expect(hits).toBe(2);
  });

  it("empty content gives zero total", () => {
    expect(matchCount("", "anything")).toEqual([0, 0]);
  });

  it("empty content blank query", () => {
    expect(matchCount("", "")).toEqual([0, 0]);
  });
});

describe("logViewer — highlightSegments", () => {
  it("blank query yields a single unmatched segment", () => {
    expect(highlightSegments("hello world", "")).toEqual([[false, "hello world"]]);
    expect(highlightSegments("hello world", "   ")).toEqual([[false, "hello world"]]);
  });

  it("single case insensitive match in the middle", () => {
    expect(highlightSegments("see ERROR here", "error")).toEqual([
      [false, "see "],
      [true, "ERROR"],
      [false, " here"],
    ]);
  });

  it("multiple matches are all highlighted", () => {
    expect(highlightSegments("foo bar foo", "foo")).toEqual([
      [true, "foo"],
      [false, " bar "],
      [true, "foo"],
    ]);
  });

  it("no match yields a single unmatched segment", () => {
    expect(highlightSegments("hello world", "zzz")).toEqual([[false, "hello world"]]);
  });

  it("match spanning the whole text yields one segment", () => {
    expect(highlightSegments("foo", "foo")).toEqual([[true, "foo"]]);
  });

  it("regression: case folding that shrinks length does not misalign", () => {
    // `ẞ` (capital sharp S) lowercases to `ß`; slicing must stay aligned to `text`'s own
    // char boundaries rather than a lowercased copy's.
    expect(highlightSegments("ẞzz", "zz")).toEqual([
      [false, "ẞ"],
      [true, "zz"],
    ]);
  });

  it("regression: case folding that grows length does not misalign", () => {
    // Turkish `İ` lowercases to a 2-codepoint sequence (`i` + combining dot above).
    expect(highlightSegments("İzz", "zz")).toEqual([
      [false, "İ"],
      [true, "zz"],
    ]);
  });

  it("regression: a match starting on an expanding character is still found", () => {
    // Unlike the previous case, the match here *starts on* `İ` itself, so a lowercased copy
    // that isn't truncated back to one code point per original character throws off every
    // window position for the rest of the match — the whole "istanbul" span was silently
    // left unmatched before this was fixed.
    expect(highlightSegments("İstanbul job failed", "istanbul")).toEqual([
      [true, "İstanbul"],
      [false, " job failed"],
    ]);
  });
});
