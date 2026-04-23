// Unit tests for the in-repo minimal markdown parser. Covers the documented
// scope (headings, lists, fenced code, paragraphs, inline code, links).

import { describe, expect, it } from "vitest";
import {
  parseInline,
  parseMarkdown,
} from "../../src/viewers/markdown-parser";

describe("parseMarkdown", () => {
  it("parses heading levels 1 through 3", () => {
    const nodes = parseMarkdown("# one\n## two\n### three");
    expect(nodes).toHaveLength(3);
    expect(nodes[0]).toMatchObject({ kind: "heading", level: 1 });
    expect(nodes[1]).toMatchObject({ kind: "heading", level: 2 });
    expect(nodes[2]).toMatchObject({ kind: "heading", level: 3 });
  });

  it("groups unordered list items until blank line", () => {
    const nodes = parseMarkdown("* a\n* b\n- c\n\nparagraph");
    expect(nodes).toHaveLength(2);
    expect(nodes[0]).toMatchObject({
      kind: "list",
      items: [
        [{ kind: "text", text: "a" }],
        [{ kind: "text", text: "b" }],
        [{ kind: "text", text: "c" }],
      ],
    });
    expect(nodes[1]).toMatchObject({ kind: "paragraph" });
  });

  it("parses fenced code blocks and preserves language hint", () => {
    const source = "```ts\nconst x = 1;\n```\n";
    const nodes = parseMarkdown(source);
    expect(nodes).toEqual([
      { kind: "code", lang: "ts", text: "const x = 1;" },
    ]);
  });

  it("joins wrapped paragraph lines with a space", () => {
    const nodes = parseMarkdown("line one\nline two\n\nthird");
    expect(nodes).toHaveLength(2);
    expect(nodes[0]).toMatchObject({
      kind: "paragraph",
      children: [{ kind: "text", text: "line one line two" }],
    });
  });

  it("handles an unterminated fence without crashing", () => {
    const nodes = parseMarkdown("```rs\nfn x() {}\n");
    expect(nodes).toEqual([
      { kind: "code", lang: "rs", text: "fn x() {}\n" },
    ]);
  });
});

describe("parseInline", () => {
  it("recognises inline code, links, and plain text", () => {
    const parts = parseInline("see `foo` [doc](https://x) end");
    expect(parts).toEqual([
      { kind: "text", text: "see " },
      { kind: "code", text: "foo" },
      { kind: "text", text: " " },
      { kind: "link", text: "doc", url: "https://x" },
      { kind: "text", text: " end" },
    ]);
  });

  it("treats an unmatched backtick as plain text", () => {
    const parts = parseInline("x `y");
    expect(parts).toEqual([{ kind: "text", text: "x `y" }]);
  });

  it("treats a malformed link as plain text", () => {
    const parts = parseInline("[oops](broken");
    expect(parts).toEqual([{ kind: "text", text: "[oops](broken" }]);
  });
});
