// Image viewer: FileRead → Blob → object URL; provides scale + pan.
// Sanity checks: reject payloads > 20 MiB, warn when natural size exceeds
// 4096×4096. Object URL is revoked on unmount or path change.

import { useCallback, useEffect, useRef, useState } from "react";
import { WasmClient } from "../wasm-bridge";
import {
  decodeFileRead,
  describeFileReadError,
} from "./file-read-decoder";

const MAX_BYTES = 20 * 1024 * 1024;
const WARN_DIM = 4096;

type ImageViewerProps = {
  path: unknown;
  client: WasmClient;
};

type NaturalSize = { width: number; height: number } | null;

export function ImageViewer({ path, client }: ImageViewerProps) {
  const [url, setUrl] = useState<string | null>(null);
  const [byteSize, setByteSize] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [natural, setNatural] = useState<NaturalSize>(null);
  const [scale, setScale] = useState<number>(1);
  const [offset, setOffset] = useState<{ x: number; y: number }>({ x: 0, y: 0 });
  const dragRef = useRef<{
    startX: number;
    startY: number;
    origX: number;
    origY: number;
  } | null>(null);

  useEffect(() => {
    let cancelled = false;
    let currentUrl: string | null = null;
    setUrl(null);
    setError(null);
    setByteSize(null);
    setNatural(null);
    setScale(1);
    setOffset({ x: 0, y: 0 });
    client
      .dispatchFileRead({ path })
      .then((response) => {
        if (cancelled) return;
        const decoded = decodeFileRead(response);
        if (!decoded || decoded.kind !== "binary") {
          setError("unexpected image payload");
          return;
        }
        if (decoded.bytes.length > MAX_BYTES) {
          setError(`image too large: ${decoded.bytes.length} bytes`);
          return;
        }
        // Copy into a fresh ArrayBuffer so Blob typing (BlobPart expects an
        // ArrayBuffer-backed view, not SharedArrayBuffer) stays happy.
        const buffer = new Uint8Array(decoded.bytes.length);
        buffer.set(decoded.bytes);
        const blob = new Blob([buffer.buffer], {
          type: decoded.mediaType || "application/octet-stream",
        });
        currentUrl = URL.createObjectURL(blob);
        setUrl(currentUrl);
        setByteSize(decoded.bytes.length);
      })
      .catch((err) => {
        if (cancelled) return;
        setError(`failed to read: ${describeFileReadError(err)}`);
      });
    return () => {
      cancelled = true;
      if (currentUrl) URL.revokeObjectURL(currentUrl);
    };
  }, [client, path]);

  const handleLoad = useCallback(
    (event: React.SyntheticEvent<HTMLImageElement>) => {
      const img = event.currentTarget;
      setNatural({ width: img.naturalWidth, height: img.naturalHeight });
      if (img.naturalWidth > WARN_DIM || img.naturalHeight > WARN_DIM) {
        console.warn(
          `image dimensions exceed ${WARN_DIM}:`,
          img.naturalWidth,
          img.naturalHeight,
        );
      }
    },
    [],
  );

  const onPointerDown = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      dragRef.current = {
        startX: event.clientX,
        startY: event.clientY,
        origX: offset.x,
        origY: offset.y,
      };
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [offset.x, offset.y],
  );
  const onPointerMove = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      const state = dragRef.current;
      if (!state) return;
      setOffset({
        x: state.origX + (event.clientX - state.startX),
        y: state.origY + (event.clientY - state.startY),
      });
    },
    [],
  );
  const onPointerUp = useCallback(
    (event: React.PointerEvent<HTMLDivElement>) => {
      dragRef.current = null;
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
    },
    [],
  );

  return (
    <div className="viewer viewer--image" data-testid="image-viewer">
      <header className="viewer__chrome">
        <div className="viewer__chrome-stats">
          <span data-testid="image-size">
            {byteSize == null ? "—" : `${byteSize} bytes`}
            {natural ? ` · ${natural.width}×${natural.height}` : ""}
          </span>
          <span data-testid="image-scale">{`scale ${scale.toFixed(2)}×`}</span>
        </div>
        <div className="viewer__chrome-actions">
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => setScale((s) => Math.min(s * 1.25, 16))}
            data-testid="image-zoom-in"
          >
            +
          </button>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => setScale((s) => Math.max(s / 1.25, 0.1))}
            data-testid="image-zoom-out"
          >
            −
          </button>
          <button
            type="button"
            className="btn btn--ghost btn--sm"
            onClick={() => {
              setScale(1);
              setOffset({ x: 0, y: 0 });
            }}
            data-testid="image-reset"
          >
            reset
          </button>
        </div>
      </header>
      <div
        className="viewer__stage"
        onPointerDown={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onPointerCancel={onPointerUp}
      >
        {error ? (
          <p className="viewer__error" data-testid="image-error">
            {error}
          </p>
        ) : url ? (
          <img
            src={url}
            alt=""
            onLoad={handleLoad}
            draggable={false}
            style={{
              transform: `translate(${offset.x}px, ${offset.y}px) scale(${scale})`,
              transformOrigin: "center center",
            }}
            data-testid="image-element"
          />
        ) : (
          <p className="viewer__loading">reading…</p>
        )}
      </div>
    </div>
  );
}
