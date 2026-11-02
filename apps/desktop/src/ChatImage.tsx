import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { ImageOff, LoaderCircle, X } from "lucide-react";
import { useFileNavigation } from "./FileNavigation.js";
import { fileReference } from "./fileContext.js";
import { inlineImage, mediaHash, type ChatImageSource } from "./chatMedia.js";

function ImagePreview({
  src,
  alt,
  close,
}: ChatImageSource & { close: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const previous = document.activeElement;
    dialog.current!.showModal();
    return () => {
      dialog.current?.close();
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
      <button
        type="button"
        className="icon-button"
        aria-label="Close image preview"
        onClick={close}
      >
        <X size={18} />
      </button>
      <img src={src} alt={alt} />
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
