// Markdown viewer: dispatches FileRead, renders through the in-repo minimal
// parser. Contents stay in local state — they are not promoted into the
// Zustand UI store.

import { useEffect, useState } from "react";
import { WasmClient } from "../wasm-bridge";
import {
  decodeFileRead,
  describeFileReadError,
} from "./file-read-decoder";
import { parseMarkdown, type MdInline, type MdNode } from "./markdown-parser";

type MarkdownViewerProps = {
  path: unknown;
  client: WasmClient;
};

export function MarkdownViewer({ path, client }: MarkdownViewerProps) {
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    setText(null);
    setError(null);
    client
      .dispatchFileRead({ path })
      .then((response) => {
        if (cancelled) return;
        const decoded = decodeFileRead(response);
        if (!decoded) {
          setError("unexpected FileRead payload");
          return;
        }
        if (decoded.kind !== "utf8") {
          setError("markdown file is not utf-8 text");
          return;
        }
        setText(decoded.text);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(`failed to read: ${describeFileReadError(err)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [client, path]);

  return (
    <div className="viewer viewer--markdown" data-testid="markdown-viewer">
      {error ? (
        <p className="viewer__error" data-testid="markdown-error">
          {error}
        </p>
      ) : text == null ? (
        <p className="viewer__loading">reading…</p>
      ) : (
        <article
          className="markdown-body"
          data-testid="markdown-body"
        >
          {parseMarkdown(text).map((node, i) => (
            <MarkdownNode key={i} node={node} />
          ))}
        </article>
      )}
    </div>
  );
}

function MarkdownNode({ node }: { node: MdNode }) {
  if (node.kind === "heading") {
    if (node.level === 1)
      return <h1><InlineRun inline={node.children} /></h1>;
    if (node.level === 2)
      return <h2><InlineRun inline={node.children} /></h2>;
    return <h3><InlineRun inline={node.children} /></h3>;
  }
  if (node.kind === "paragraph") {
    return (
      <p>
        <InlineRun inline={node.children} />
      </p>
    );
  }
  if (node.kind === "list") {
    return (
      <ul>
        {node.items.map((item, i) => (
          <li key={i}>
            <InlineRun inline={item} />
          </li>
        ))}
      </ul>
    );
  }
  return (
    <pre className="markdown-body__code" data-lang={node.lang}>
      <code>{node.text}</code>
    </pre>
  );
}

function InlineRun({ inline }: { inline: MdInline[] }) {
  return (
    <>
      {inline.map((part, i) => {
        if (part.kind === "text") return <span key={i}>{part.text}</span>;
        if (part.kind === "code") return <code key={i}>{part.text}</code>;
        return (
          <a key={i} href={part.url} target="_blank" rel="noreferrer">
            {part.text}
          </a>
        );
      })}
    </>
  );
}
