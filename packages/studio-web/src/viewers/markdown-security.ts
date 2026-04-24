import { defaultSchema, type Options as SanitizeSchema } from "rehype-sanitize";

const SAFE_LINK_SCHEMES = new Set(["http:", "https:", "mailto:", "tel:"]);
const SAFE_IMAGE_SCHEMES = new Set(["http:", "https:"]);

type UrlOptions = {
  image?: boolean;
};

export const mermaidSecurityConfig = {
  startOnLoad: false,
  securityLevel: "strict",
} as const;

export const markdownSanitizeSchema: SanitizeSchema = {
  ...defaultSchema,
  attributes: {
    ...defaultSchema.attributes,
    a: includeAttributes(defaultSchema.attributes?.a, ["href", "target", "rel"]),
    code: includeAttributes(defaultSchema.attributes?.code, ["className"]),
    div: includeAttributes(defaultSchema.attributes?.div, [
      "className",
      "data-testid",
      "data-color-mode",
    ]),
    img: includeAttributes(defaultSchema.attributes?.img, [
      "src",
      "alt",
      "title",
      "width",
      "height",
    ]),
  },
};

export function isSafeMarkdownUrl(
  value: string | null | undefined,
  options: UrlOptions = {},
): boolean {
  if (!value) return false;
  const trimmed = value.trim();
  if (trimmed.length === 0) return false;
  if (
    trimmed.startsWith("#") ||
    trimmed.startsWith("/") ||
    trimmed.startsWith("./") ||
    trimmed.startsWith("../")
  ) {
    return true;
  }
  try {
    const parsed = new URL(trimmed, "https://budn.local/");
    if (parsed.origin === "https://budn.local" && !trimmed.includes(":")) {
      return true;
    }
    const protocol = parsed.protocol.toLowerCase();
    return options.image
      ? SAFE_IMAGE_SCHEMES.has(protocol)
      : SAFE_LINK_SCHEMES.has(protocol);
  } catch {
    return false;
  }
}

export function markdownLinkProps(href: string | null | undefined) {
  return {
    href: safeMarkdownHref(href),
    rel: "noopener noreferrer",
    target: "_blank",
  };
}

function safeMarkdownHref(href: string | null | undefined): string {
  if (!isSafeMarkdownUrl(href)) return "#";
  const trimmed = href?.trim() ?? "";
  try {
    const parsed = new URL(trimmed);
    return parsed.href;
  } catch {
    return trimmed || "#";
  }
}

export function sanitizeMermaidSvg(svg: string): string | null {
  const parser = new DOMParser();
  const doc = parser.parseFromString(svg, "image/svg+xml");
  if (doc.querySelector("parsererror")) return null;
  const root = doc.documentElement;
  if (!root || root.tagName.toLowerCase() !== "svg") return null;
  if (root.querySelector("script")) return null;
  for (const element of Array.from(root.querySelectorAll("*"))) {
    for (const attr of Array.from(element.attributes)) {
      const name = attr.name.toLowerCase();
      if (name.startsWith("on")) return null;
      if ((name === "href" || name === "xlink:href") && !isSafeMarkdownUrl(attr.value)) {
        return null;
      }
    }
  }
  return new XMLSerializer().serializeToString(root);
}

function includeAttributes(
  base: NonNullable<SanitizeSchema["attributes"]>[string] | undefined,
  additions: string[],
): NonNullable<SanitizeSchema["attributes"]>[string] {
  const existing = base ?? [];
  const result = [...existing];
  for (const item of additions) {
    if (!result.includes(item)) result.push(item);
  }
  return result;
}
