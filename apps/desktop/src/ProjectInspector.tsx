import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Maximize2, Minimize2, X } from "lucide-react";
import { FilesPanel } from "./FilesPanel.js";
import { GitReviewPanel } from "./GitReviewPanel.js";
import type { AccentColor } from "./accentColors.js";
import type { ProjectInfo } from "./workspaceState.js";
import type { FileContext, FileLocation } from "./fileContext.js";
import type { FileSearchRequest } from "./FileSearch.js";

export type InspectorRequest = {
  project: string;
  id: number;
  location?: FileLocation;
  search?: FileSearchRequest;
};

export function ProjectInspector({
  project,
  mode,
  changeMode,
  expanded,
  setExpanded,
  close,
  working,
  theme,
  accentColor,
  request,
  attach,
  canAttach,
}: {
  project: ProjectInfo;
  mode: "files" | "changes";
  changeMode: (mode: "files" | "changes") => void;
  expanded: boolean;
  setExpanded: (value: boolean) => void;
  close: () => void;
  working: boolean;
  theme: "dark" | "light";
  accentColor: AccentColor;
  request?: InspectorRequest;
  attach: (file: FileContext, ask?: boolean) => void;
  canAttach: boolean;
}) {
  const [width, setWidth] = useState(() => {
    try {
      return Math.max(
        280,
        Math.min(
          1000,
          Number(localStorage.getItem("caelum.inspector.width")) || 460,
        ),
      );
    } catch {
      return 460;
    }
  });
  const pane = useRef<HTMLElement>(null);
  const [maximum, setMaximum] = useState(1000);
  useEffect(() => {
    const container = pane.current?.parentElement;
    if (!container) return;
    const observer = new ResizeObserver(() =>
      setMaximum(Math.min(1000, Math.max(280, container.clientWidth - 300))),
    );
    observer.observe(container);
    return () => observer.disconnect();
  }, []);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const resize = (value: number) => {
    const next = Math.max(280, Math.min(maximum, value));
    setWidth(next);
    try {
      localStorage.setItem("caelum.inspector.width", String(next));
    } catch {
      /* Resizing still works for this window. */
    }
  };
  return (
    <aside
      ref={pane}
      id="changes-pane"
      className={`changes-pane project-inspector${expanded ? " expanded" : ""}`}
      aria-label={mode === "files" ? "Project files" : "Project changes"}
      style={{ "--inspector-width": `${width}px` } as CSSProperties}
    >
      {!expanded && (
        <div
          className="inspector-resize"
          role="separator"
          aria-label="Resize project inspector"
          aria-orientation="vertical"
          aria-valuemin={280}
          aria-valuemax={maximum}
          aria-valuenow={Math.round(Math.min(width, maximum))}
          tabIndex={0}
          onPointerDown={(event) => {
            if (event.button !== 0) return;
            drag.current = {
              x: event.clientX,
              width: pane.current!.getBoundingClientRect().width,
            };
            event.currentTarget.setPointerCapture(event.pointerId);
            event.preventDefault();
          }}
          onPointerMove={(event) => {
            if (drag.current)
              resize(drag.current.width + drag.current.x - event.clientX);
          }}
          onPointerUp={() => {
            drag.current = null;
          }}
          onLostPointerCapture={() => {
            drag.current = null;
          }}
          onKeyDown={(event) => {
            if (
              ["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)
            ) {
              event.preventDefault();
              resize(
                event.key === "Home"
                  ? 280
                  : event.key === "End"
                    ? 1000
                    : Math.min(width, maximum) +
                      (event.key === "ArrowLeft" ? 32 : -32),
              );
            }
          }}
        />
      )}
      <header className="inspector-heading">
        <div
          role="tablist"
          aria-label="Project inspector"
          onKeyDown={(event) => {
            if (
              ["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)
            ) {
              event.preventDefault();
              const next =
                event.key === "Home"
                  ? "files"
                  : event.key === "End"
                    ? "changes"
                    : mode === "files"
                      ? "changes"
                      : "files";
              changeMode(next);
              event.currentTarget
                .querySelectorAll<HTMLButtonElement>('[role="tab"]')
                [next === "files" ? 0 : 1]?.focus();
            }
          }}
        >
          <button
            role="tab"
            aria-selected={mode === "files"}
            tabIndex={mode === "files" ? 0 : -1}
            onClick={() => changeMode("files")}
          >
            Files
          </button>
          <button
            role="tab"
            aria-selected={mode === "changes"}
            tabIndex={mode === "changes" ? 0 : -1}
            onClick={() => changeMode("changes")}
          >
            Changes
          </button>
        </div>
        <button
          className="icon-button"
          aria-label={
            expanded
              ? "Restore conversation layout"
              : "Expand project inspector"
          }
          aria-pressed={expanded}
          onClick={() => setExpanded(!expanded)}
        >
          {expanded ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
        </button>
        <button
          className="icon-button"
          aria-label={`Close ${mode}`}
          onClick={close}
        >
          <X size={14} />
        </button>
      </header>
      {mode === "files" ? (
        <FilesPanel
          key={project.path}
          project={project}
          theme={theme}
          accentColor={accentColor}
          working={working}
          request={request?.project === project.path ? request : undefined}
          attach={attach}
          canAttach={canAttach}
        />
      ) : (
        <GitReviewPanel
          key={project.path}
          project={project}
          working={working}
          close={close}
          embedded
        />
      )}
    </aside>
  );
}
