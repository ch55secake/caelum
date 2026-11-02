import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ImageOff, LoaderCircle, Minus, Plus, X } from "lucide-react";
import { useFileNavigation } from "../files/FileNavigation.js";
import { fileReference } from "../../domain/files/fileContext.js";
import { inlineImage, mediaHash, type ChatImageSource } from "../../domain/conversation/chatMedia.js";

function ImagePreview({
  src,
  alt,
  close,
}: ChatImageSource & { close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const viewport = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; left: number; top: number } | null>(null);
  const [panning, setPanning] = useState(false);
  const [natural, setNatural] = useState({ width: 0, height: 0 });
  const [size, setSize] = useState({ width: 0, height: 0 });
  const [zoom, setZoom] = useState<number | null>(null);
  const fit = natural.width && size.width
    ? Math.min(1, Math.max(1, size.width - 48) / natural.width, Math.max(1, size.height - 48) / natural.height)
    : 1;
  const scale = zoom ?? fit;
  const width = natural.width * scale;
  const height = natural.height * scale;
  const canPan = width + 48 > size.width || height + 48 > size.height;
  useLayoutEffect(() => {
    const element = viewport.current!;
    const observer = new ResizeObserver(([entry]) => {
      setSize({ width: entry.contentRect.width, height: entry.contentRect.height });
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  useLayoutEffect(() => {
    const element = viewport.current!;
    element.scrollLeft = (element.scrollWidth - element.clientWidth) / 2;
    element.scrollTop = (element.scrollHeight - element.clientHeight) / 2;
  }, [scale]);
  useEffect(() => {
    const previous = document.activeElement;
    const element = dialog.current!;
    element.showModal();
    return () => {
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus();
    };
  }, []);
  return createPortal(
    <dialog
      ref={dialog}
      className="chat-image-dialog"
      aria-label="Image preview"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
      onClick={(event) => {
        if (event.target === event.currentTarget) close();
      }}
    >
      <header className="chat-image-toolbar">
        <strong title={alt}>{alt}</strong>
        <div className="chat-image-controls" aria-label="Image zoom">
          <button type="button" aria-pressed={zoom === null} onClick={() => setZoom(null)}>Fit</button>
          <button type="button" aria-label="Actual size" aria-pressed={zoom === 1} onClick={() => setZoom(1)}>100%</button>
          <button type="button" className="icon-button" aria-label="Zoom out" disabled={scale <= 0.1} onClick={() => setZoom(Math.max(0.1, scale / 1.5))}><Minus size={15} /></button>
          <output aria-label="Zoom level">{Math.round(scale * 100)}%</output>
          <button type="button" className="icon-button" aria-label="Zoom in" disabled={scale >= 4} onClick={() => setZoom(Math.min(4, scale * 1.5))}><Plus size={15} /></button>
          <button type="button" className="icon-button" aria-label="Close image preview" onClick={close}><X size={18} /></button>
        </div>
      </header>
      <div
        ref={viewport}
        className="chat-image-viewport"
        tabIndex={0}
        role="region"
        aria-label="Image canvas"
        data-pannable={canPan}
        data-panning={panning}
        onPointerDown={(event) => {
          if (!canPan || event.button !== 0) return;
          event.preventDefault();
          event.currentTarget.focus();
          event.currentTarget.setPointerCapture(event.pointerId);
          drag.current = { x: event.clientX, y: event.clientY, left: event.currentTarget.scrollLeft, top: event.currentTarget.scrollTop };
          setPanning(true);
        }}
        onPointerMove={(event) => {
          if (!drag.current) return;
          event.currentTarget.scrollLeft = drag.current.left + drag.current.x - event.clientX;
          event.currentTarget.scrollTop = drag.current.top + drag.current.y - event.clientY;
        }}
        onPointerUp={(event) => {
          if (event.currentTarget.hasPointerCapture(event.pointerId))
            event.currentTarget.releasePointerCapture(event.pointerId);
        }}
        onLostPointerCapture={() => { drag.current = null; setPanning(false); }}
      >
        <div className="chat-image-canvas" style={{ width: width + 48, height: height + 48 }}>
          <img
            src={src}
            alt={alt}
            draggable={false}
            style={natural.width ? { width, height } : undefined}
            onLoad={(event) => setNatural({ width: event.currentTarget.naturalWidth, height: event.currentTarget.naturalHeight })}
          />
        </div>
      </div>
    </dialog>,
    document.body,
  );
}

export function ChatImage({ src, alt = "Image" }: ChatImageSource) {
  const navigation = useFileNavigation();
  const projectPath = navigation?.projectPath;
  const [resolved, setResolved] = useState<{
    source: string;
    url?: string;
    error?: string;
  }>();
  const [loaded, setLoaded] = useState("");
  const [expanded, setExpanded] = useState(false);
  const source = `${projectPath}\0${src}`;
  useEffect(() => {
    let disposed = false;
    setExpanded(false);
    const resolve = async () => {
      if (/^https?:\/\//i.test(src) || inlineImage(src)) return src;
      const hash = mediaHash(src);
      if (hash && projectPath)
        return invoke<string>("opencode_read_image", { projectPath, hash });
      const file = projectPath && fileReference(src, projectPath);
      if (file)
        return invoke<string>("read_project_image", {
          projectPath,
          path: file.path,
        });
      throw new Error("Image source is unavailable");
    };
    void resolve().then(
      (url) => {
        if (!disposed) setResolved({ source, url });
      },
      (error) => {
        if (!disposed) setResolved({ source, error: String(error) });
      },
    );
    return () => {
      disposed = true;
    };
  }, [src, projectPath, source]);
  const current = resolved?.source === source ? resolved : undefined;
  return (
    <span className="chat-image">
      {current?.error ? (
        <span className="chat-image-error" title={current.error}>
          <ImageOff size={16} />
          {alt} · Image unavailable
        </span>
      ) : (
        <button
          type="button"
          className="chat-image-thumbnail"
          aria-label={`Expand ${alt}`}
          disabled={!current?.url || loaded !== source}
          onClick={() => setExpanded(true)}
        >
          {loaded !== source && (
            <span className="chat-image-loading">
              <LoaderCircle size={16} className="spinning" />
              Loading image…
            </span>
          )}
          {current?.url && (
            <img
              src={current.url}
              alt={alt}
              loading="lazy"
              decoding="async"
              onLoad={() => setLoaded(source)}
              onError={() =>
                setResolved({ source, error: "Could not load image" })
              }
            />
          )}
        </button>
      )}
      {expanded && current?.url && (
        <ImagePreview
          src={current.url}
          alt={alt}
          close={() => setExpanded(false)}
        />
      )}
    </span>
  );
}

export function ChatImages({ images }: { images?: ChatImageSource[] }) {
  return images?.length ? (
    <div className="chat-images">
      {images.map((image, index) => (
        <ChatImage key={`${index}:${image.src}`} {...image} />
      ))}
    </div>
  ) : null;
}
