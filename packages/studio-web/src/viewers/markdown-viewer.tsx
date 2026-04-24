import MarkdownPreview, {
  type MarkdownPreviewProps,
} from "@uiw/react-markdown-preview";
import rehypeSanitize from "rehype-sanitize";
import { isValidElement, useEffect, useRef, useState } from "react";
import type { ReactNode } from "react";
import { WasmClient } from "../wasm-bridge";
import {
  decodeFileRead,
  describeFileReadError,
} from "./file-read-decoder";
import {
  isSafeMarkdownUrl,
  markdownLinkProps,
  markdownSanitizeSchema,
  mermaidSecurityConfig,
  sanitizeMermaidSvg,
} from "./markdown-security";

type MarkdownViewerProps = {
  path: unknown;
  client: WasmClient;
  refreshSignal?: number;
};

let mermaidCounter = 0;

const markdownWrapperElement = {
  "data-color-mode": "dark",
  "data-testid": "markdown-preview",
} as MarkdownPreviewProps["wrapperElement"];

export function MarkdownViewer({
  path,
  client,
  refreshSignal,
}: MarkdownViewerProps) {
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
  }, [client, path, refreshSignal]);

  return (
    <div className="viewer viewer--markdown" data-testid="markdown-viewer">
      {error ? (
        <p className="viewer__error" data-testid="markdown-error">
          {error}
        </p>
      ) : text == null ? (
        <p className="viewer__loading">reading…</p>
      ) : (
        <article className="markdown-body" data-testid="markdown-body">
          <MarkdownPreview
            source={text}
            wrapperElement={markdownWrapperElement}
            className="markdown-preview"
            rehypePlugins={[[rehypeSanitize, markdownSanitizeSchema]]}
            components={markdownComponents}
          />
        </article>
      )}
    </div>
  );
}

const markdownComponents: MarkdownPreviewProps["components"] = {
  a({ href, children, ...props }) {
    return (
      <a {...props} {...markdownLinkProps(href)}>
        {children}
      </a>
    );
  },
  img({ src, alt, ...props }) {
    if (!isSafeMarkdownUrl(src, { image: true })) return null;
    return <img {...props} src={src} alt={alt ?? ""} loading="lazy" />;
  },
  code({ className, children, ...props }) {
    const source = textFromReactNode(children).trim();
    if (/\blanguage-mermaid\b/.test(className ?? "")) {
      return <MermaidBlock source={source} />;
    }
    return (
      <code className={className} {...props}>
        {children}
      </code>
    );
  },
};

function MermaidBlock({ source }: { source: string }) {
  const idRef = useRef<string | null>(null);
  const [svg, setSvg] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  if (!idRef.current) {
    mermaidCounter += 1;
    idRef.current = `budn-markdown-mermaid-${mermaidCounter}`;
  }

  useEffect(() => {
    let cancelled = false;
    setSvg(null);
    setError(null);
    import("mermaid")
      .then((result) => {
        if (cancelled) return;
        const mermaid = result.default;
        mermaid.initialize(mermaidSecurityConfig);
        return mermaid.render(idRef.current ?? "budn-markdown-mermaid", source);
      })
      .then((result) => {
        if (cancelled || !result) return;
        const safeSvg = sanitizeMermaidSvg(result.svg);
        if (!safeSvg) {
          setError("unsafe Mermaid output");
          return;
        }
        setSvg(safeSvg);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(describeFileReadError(err));
      });
    return () => {
      cancelled = true;
    };
  }, [source]);

  if (error) {
    return (
      <pre
        className="markdown-mermaid markdown-mermaid--error"
        data-testid="markdown-mermaid"
      >
        <code>{error}</code>
      </pre>
    );
  }
  if (!svg) {
    return (
      <div className="markdown-mermaid" data-testid="markdown-mermaid">
        rendering diagram…
      </div>
    );
  }
  return (
    <div
      className="markdown-mermaid"
      data-testid="markdown-mermaid"
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  );
}

function textFromReactNode(node: ReactNode): string {
  if (node == null || typeof node === "boolean") return "";
  if (typeof node === "string" || typeof node === "number") return String(node);
  if (Array.isArray(node)) return node.map(textFromReactNode).join("");
  if (isValidElement<{ children?: ReactNode }>(node)) {
    return textFromReactNode(node.props.children);
  }
  return "";
}
