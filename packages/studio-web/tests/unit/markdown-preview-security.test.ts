import { describe, expect, it } from "vitest";
import {
  isSafeMarkdownUrl,
  markdownLinkProps,
  markdownSanitizeSchema,
  mermaidSecurityConfig,
  sanitizeMermaidSvg,
} from "../../src/viewers/markdown-security";

describe("markdown preview security", () => {
  it("allows common safe links and rejects script URLs", () => {
    expect(isSafeMarkdownUrl("https://example.com")).toBe(true);
    expect(isSafeMarkdownUrl("http://example.com")).toBe(true);
    expect(isSafeMarkdownUrl("mailto:team@example.com")).toBe(true);
    expect(isSafeMarkdownUrl("/docs/readme.md")).toBe(true);
    expect(isSafeMarkdownUrl("#section")).toBe(true);
    expect(isSafeMarkdownUrl("javascript:alert(1)")).toBe(false);
    expect(isSafeMarkdownUrl("data:text/html,<script>alert(1)</script>")).toBe(false);
    expect(isSafeMarkdownUrl("javascript:alert(1)", { image: true })).toBe(false);
    expect(isSafeMarkdownUrl("data:image/svg+xml,<svg onload=alert(1)>", { image: true })).toBe(false);
  });

  it("opens markdown links in a new browser tab", () => {
    expect(markdownLinkProps("https://example.com")).toEqual({
      href: "https://example.com/",
      rel: "noopener noreferrer",
      target: "_blank",
    });
  });

  it("keeps target and rel attributes available after sanitization", () => {
    const anchorAttributes = markdownSanitizeSchema.attributes?.a ?? [];
    expect(anchorAttributes).toContain("href");
    expect(anchorAttributes).toContain("target");
    expect(anchorAttributes).toContain("rel");
  });

  it("uses strict Mermaid rendering settings", () => {
    expect(mermaidSecurityConfig.securityLevel).toBe("strict");
    expect(mermaidSecurityConfig.startOnLoad).toBe(false);
  });

  it("rejects Mermaid SVG output with scripts or inline handlers", () => {
    expect(sanitizeMermaidSvg("<svg><text>ok</text></svg>")).toContain("<svg");
    expect(sanitizeMermaidSvg("<svg><script>alert(1)</script></svg>")).toBeNull();
    expect(sanitizeMermaidSvg("<svg><a onclick=\"alert(1)\">bad</a></svg>")).toBeNull();
    expect(
      sanitizeMermaidSvg("<svg><a href=\"javascript:alert(1)\">bad</a></svg>"),
    ).toBeNull();
    expect(
      sanitizeMermaidSvg(
        "<svg><a xlink:href=\"javascript:alert(1)\">bad</a></svg>",
      ),
    ).toBeNull();
  });
});
