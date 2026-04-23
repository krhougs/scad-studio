// Minimal markdown parser used by MarkdownViewer. Scope is deliberately tight:
// headings (#/##/###), unordered lists (* or -), fenced code blocks (```),
// paragraphs, inline code (`…`) and links ([text](url)). This is not a GFM
// implementation; limits are listed in docs/web-platform-limits.md.

export type MdNode =
  | { kind: "heading"; level: 1 | 2 | 3; children: MdInline[] }
  | { kind: "paragraph"; children: MdInline[] }
  | { kind: "list"; items: MdInline[][] }
  | { kind: "code"; lang: string; text: string };

export type MdInline =
  | { kind: "text"; text: string }
  | { kind: "code"; text: string }
  | { kind: "link"; text: string; url: string };

export function parseMarkdown(source: string): MdNode[] {
  const lines = source.replace(/\r\n/g, "\n").split("\n");
  const nodes: MdNode[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (line.startsWith("```")) {
      const lang = line.slice(3).trim();
      const buf: string[] = [];
      i += 1;
      while (i < lines.length && !lines[i]!.startsWith("```")) {
        buf.push(lines[i]!);
        i += 1;
      }
      if (i < lines.length) i += 1; // skip closing fence
      nodes.push({ kind: "code", lang, text: buf.join("\n") });
      continue;
    }
    const heading = matchHeading(line);
    if (heading) {
      nodes.push({
        kind: "heading",
        level: heading.level,
        children: parseInline(heading.text),
      });
      i += 1;
      continue;
    }
    if (isListStart(line)) {
      const items: MdInline[][] = [];
      while (i < lines.length && isListStart(lines[i]!)) {
        const stripped = lines[i]!.replace(/^\s*[*-]\s+/, "");
        items.push(parseInline(stripped));
        i += 1;
      }
      nodes.push({ kind: "list", items });
      continue;
    }
    if (line.trim() === "") {
      i += 1;
      continue;
    }
    const paragraphLines: string[] = [line];
    i += 1;
    while (
      i < lines.length &&
      lines[i]!.trim() !== "" &&
      !matchHeading(lines[i]!) &&
      !isListStart(lines[i]!) &&
      !lines[i]!.startsWith("```")
    ) {
      paragraphLines.push(lines[i]!);
      i += 1;
    }
    nodes.push({
      kind: "paragraph",
      children: parseInline(paragraphLines.join(" ")),
    });
  }
  return nodes;
}

function matchHeading(
  line: string,
): { level: 1 | 2 | 3; text: string } | null {
  const m = /^(#{1,3})\s+(.+)$/.exec(line);
  if (!m) return null;
  const level = m[1]!.length as 1 | 2 | 3;
  return { level, text: m[2]! };
}

function isListStart(line: string): boolean {
  return /^\s*[*-]\s+/.test(line);
}

export function parseInline(source: string): MdInline[] {
  const out: MdInline[] = [];
  let i = 0;
  let buf = "";
  const flush = () => {
    if (buf.length > 0) {
      out.push({ kind: "text", text: buf });
      buf = "";
    }
  };
  while (i < source.length) {
    const ch = source[i]!;
    if (ch === "`") {
      const end = source.indexOf("`", i + 1);
      if (end > i) {
        flush();
        out.push({ kind: "code", text: source.slice(i + 1, end) });
        i = end + 1;
        continue;
      }
    }
    if (ch === "[") {
      const close = source.indexOf("]", i + 1);
      if (close > i && source[close + 1] === "(") {
        const urlEnd = source.indexOf(")", close + 2);
        if (urlEnd > close) {
          flush();
          out.push({
            kind: "link",
            text: source.slice(i + 1, close),
            url: source.slice(close + 2, urlEnd),
          });
          i = urlEnd + 1;
          continue;
        }
      }
    }
    buf += ch;
    i += 1;
  }
  flush();
  return out;
}
