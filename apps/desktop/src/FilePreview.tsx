import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Compartment, EditorState, type Extension } from "@codemirror/state";
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLineGutter,
  drawSelection,
} from "@codemirror/view";
import { defaultKeymap } from "@codemirror/commands";
import { search, searchKeymap } from "@codemirror/search";
import { syntaxHighlighting } from "@codemirror/language";
import { MergeView } from "@codemirror/merge";
import { codeHighlightStyle, fileLanguage } from "./codeSyntax.js";
import { copyAction, useContextMenu } from "./ContextMenu.js";
import {
  createFileContext,
  type FileContext,
  type FileLocation,
} from "./fileContext.js";
import {
  selectedFileText,
  type CodePosition,
  type FileViewState,
  type ReadingPosition,
} from "./codeViewState.js";

type SelectedCode = {
  content: string;
  startLine: number;
  endLine: number;
  original: boolean;
};
type Viewer = {
  main: EditorView;
  original?: EditorView;
  scroller: HTMLElement;
  destroy: () => void;
};

function position(view: EditorView, offset: number): CodePosition {
  const line = view.state.doc.lineAt(offset);
  return { line: line.number, column: offset - line.from + 1 };
}
function offset(view: EditorView, point: CodePosition): number {
  const line = view.state.doc.line(
    Math.min(view.state.doc.lines, Math.max(1, point.line)),
  );
  return Math.min(line.to, line.from + Math.max(0, point.column - 1));
}
function readingPosition(
  view: EditorView,
  scroller: HTMLElement,
): ReadingPosition {
  const top = scroller.getBoundingClientRect().top - view.documentTop;
  const block = view.lineBlockAtHeight(Math.max(0, top));
  const range = view.state.selection.main;
  return {
    line: view.state.doc.lineAt(block.from).number,
    offset: top - block.top,
    left: view.scrollDOM.scrollLeft,
    anchor: position(view, range.anchor),
    head: position(view, range.head),
  };
}

export default function FilePreview({
  path,
  content,
  original,
  viewState,
  saveView,
  jump,
  attach,
  canAttach,
}: {
  path: string;
  content: string;
  original?: string;
  viewState: FileViewState | null;
  saveView: (view: FileViewState) => void;
  jump?: FileLocation & { id: number };
  attach: (file: FileContext, ask?: boolean) => void;
  canAttach: boolean;
}) {
  const host = useRef<HTMLDivElement>(null);
  const instance = useRef<Viewer | null>(null);
  const latest = useRef({
    content,
    original,
    saveView,
    attach,
    canAttach,
    viewState,
  });
  latest.current = {
    content,
    original,
    saveView,
    attach,
    canAttach,
    viewState,
  };
  const restoring = useRef(false);
  const restoreFrame = useRef(0);
  const restoreRevision = useRef(0);
  const selectionSource = useRef<"main" | "original">("main");
  const [selection, setSelection] = useState<SelectedCode | null>(null);
  const contextMenu = useContextMenu();
  const lastJump = useRef<number | undefined>(undefined);

  function selectedCode(view: EditorView): SelectedCode | null {
    const range = view.state.selection.main;
    if (range.empty) return null;
    const isOriginal = view === instance.current?.original;
    const start = position(view, range.from),
      end = position(view, range.to);
    return {
      content: selectedFileText(
        isOriginal ? (latest.current.original ?? "") : latest.current.content,
        start,
        end,
      ),
      startLine: start.line,
      endLine:
        end.column === 1 && end.line > start.line ? end.line - 1 : end.line,
      original: isOriginal,
    };
  }
  function save() {
    const viewer = instance.current;
    if (!viewer || restoring.current) return;
    latest.current.saveView({
      main: readingPosition(viewer.main, viewer.scroller),
      ...(viewer.original
        ? { original: readingPosition(viewer.original, viewer.scroller) }
        : {}),
    });
  }
  function attachSelection(value: SelectedCode | null, ask = false) {
    if (value && latest.current.canAttach)
      latest.current.attach(createFileContext(path, value.content, value), ask);
  }
  function restore(saved: FileViewState | null) {
    const viewer = instance.current;
    if (!viewer) return;
    restoring.current = true;
    const revision = ++restoreRevision.current;
    cancelAnimationFrame(restoreFrame.current);
    const entries = [
      [viewer.main, saved?.main],
      [viewer.original, saved?.original],
    ] as const;
    for (const [view, state] of entries) {
      if (!view || !state) continue;
      view.dispatch({
        selection: {
          anchor: offset(view, state.anchor),
          head: offset(view, state.head),
        },
        effects:
          view === viewer.main
            ? EditorView.scrollIntoView(
                offset(view, { line: state.line, column: 1 }),
                { y: "start", yMargin: 0 },
              )
            : [],
      });
    }
    // Let the initial reveal measure its viewport before restoring pixel offsets.
    restoreFrame.current = requestAnimationFrame(() =>
      viewer.main.requestMeasure({
        key: viewer,
        read: () =>
          entries.map(([view, state]) =>
            view && state
              ? {
                  view,
                  top:
                    view.lineBlockAt(
                      offset(view, { line: state.line, column: 1 }),
                    ).top +
                    view.documentTop +
                    viewer.scroller.scrollTop -
                    viewer.scroller.getBoundingClientRect().top +
                    state.offset,
                  left: state.left,
                }
              : null,
          ),
        write: (positions) => {
          if (
            instance.current !== viewer ||
            restoreRevision.current !== revision
          )
            return;
          for (const value of positions)
            if (value) {
              // Merge panes share vertical scrolling so unchanged lines stay aligned.
              if (value.view === viewer.main)
                viewer.scroller.scrollTop = value.top;
              value.view.scrollDOM.scrollLeft = value.left;
            }
          // Dispatching or reading layout during a measurement write is forbidden.
          queueMicrotask(() => {
            if (
              instance.current !== viewer ||
              restoreRevision.current !== revision
            )
              return;
            restoring.current = false;
            setSelection(
              selectedCode(
                selectionSource.current === "original" && viewer.original
                  ? viewer.original
                  : viewer.main,
              ),
            );
            revealRef.current();
            save();
          });
        },
      }),
    );
  }
  function reveal() {
    const view = instance.current?.main;
    if (!view || !jump || lastJump.current === jump.id) return;
    lastJump.current = jump.id;
    if (jump.line) {
      const at = offset(view, { line: jump.line, column: jump.column ?? 1 });
      view.dispatch({
        selection: { anchor: at },
        effects: EditorView.scrollIntoView(at, { y: "center" }),
      });
    }
    view.focus();
  }
  const revealRef = useRef(reveal);
  revealRef.current = reveal;

  useLayoutEffect(() => {
    let disposed = false;
    const language = new Compartment();
    const extensions = (label: string): Extension[] => [
      EditorState.readOnly.of(true),
      EditorView.editable.of(false),
      EditorView.contentAttributes.of({
        "aria-label": label,
        "aria-readonly": "true",
        tabindex: "0",
      }),
      lineNumbers(),
      highlightActiveLineGutter(),
      drawSelection(),
      syntaxHighlighting(codeHighlightStyle),
      language.of([]),
      keymap.of([...searchKeymap, ...defaultKeymap]),
      search({ top: true }),
      EditorView.updateListener.of((update) => {
        if (restoring.current) return;
        if (
          update.selectionSet ||
          (update.focusChanged && update.view.hasFocus)
        ) {
          selectionSource.current =
            update.view === instance.current?.original ? "original" : "main";
          setSelection(selectedCode(update.view));
        }
        if (update.selectionSet || update.viewportChanged) save();
      }),
    ];
    if (original === undefined) {
      const view = new EditorView({
        doc: content,
        extensions: extensions(`Read-only preview of ${path}`),
        parent: host.current!,
      });
      instance.current = {
        main: view,
        scroller: view.scrollDOM,
        destroy: () => view.destroy(),
      };
    } else {
      const merge = new MergeView({
        a: {
          doc: original,
          extensions: extensions(`Read-only HEAD version of ${path}`),
        },
        b: {
          doc: content,
          extensions: extensions(`Read-only working tree version of ${path}`),
        },
        parent: host.current!,
        gutter: true,
        highlightChanges: true,
      });
      instance.current = {
        main: merge.b,
        original: merge.a,
        scroller: merge.dom,
        destroy: () => merge.destroy(),
      };
    }
    const container = host.current!;
    container.addEventListener("scroll", save, true);
    restore(latest.current.viewState);
    void fileLanguage(path)
      .then((support) => {
        if (!disposed && support) {
          const viewer = instance.current!;
          for (const view of [viewer.main, viewer.original])
            view?.dispatch({
              effects: language.reconfigure(support.extension),
            });
        }
      })
      .catch(() => {
        /* Plain text stays readable if a grammar cannot load. */
      });
    return () => {
      disposed = true;
      cancelAnimationFrame(restoreFrame.current);
      restoreRevision.current++;
      container.removeEventListener("scroll", save, true);
      const viewer = instance.current;
      instance.current = null;
      viewer?.destroy();
    };
  }, [path, original === undefined]);

  useEffect(() => {
    const viewer = instance.current;
    if (!viewer) return;
    const updates = [
      [viewer.main, content],
      [viewer.original, original],
    ] as const;
    if (
      !updates.some(
        ([view, text]) =>
          view &&
          text !== undefined &&
          view.state.doc.toString() !== text.replace(/\r\n?/g, "\n"),
      )
    ) {
      // A line-ending-only refresh still changes the exact selection snapshot.
      setSelection(
        selectedCode(
          selectionSource.current === "original" && viewer.original
            ? viewer.original
            : viewer.main,
        ),
      );
      return;
    }
    const saved = restoring.current
      ? latest.current.viewState
      : {
          main: readingPosition(viewer.main, viewer.scroller),
          ...(viewer.original
            ? { original: readingPosition(viewer.original, viewer.scroller) }
            : {}),
        };
    restoring.current = true;
    for (const [view, text] of updates)
      if (view && text !== undefined) {
        view.dispatch({
          changes: { from: 0, to: view.state.doc.length, insert: text },
        });
      }
    restore(saved);
  }, [content, original]);
  useEffect(() => {
    if (!restoring.current) revealRef.current();
  }, [jump?.id]);

  return (
    <>
      {selection && (
        <div className="file-selection-actions">
          <span>
            {selection.original ? "HEAD · " : ""}
            {selection.startLine === selection.endLine
              ? `Line ${selection.startLine}`
              : `Lines ${selection.startLine}–${selection.endLine}`}
          </span>
          <button
            disabled={!canAttach}
            onClick={() => attachSelection(selection)}
          >
            Add selection to prompt
          </button>
          <button
            disabled={!canAttach}
            onClick={() => attachSelection(selection, true)}
          >
            Ask about selection
          </button>
        </div>
      )}
      {original !== undefined && (
        <div className="file-diff-labels">
          <span>HEAD</span>
          <span>Working tree</span>
        </div>
      )}
      <div
        className="code-viewer"
        ref={host}
        onContextMenu={(event) => {
          const viewer = instance.current;
          if (!viewer) return;
          const target = event.target as Node;
          const view = viewer.original?.dom.contains(target)
            ? viewer.original
            : viewer.main;
          const selected = selectedCode(view);
          contextMenu(event, [
            ...(selected
              ? [
                  copyAction("Copy selection", selected.content),
                  {
                    text: "Add selection to prompt",
                    enabled: canAttach,
                    action: () => attachSelection(selected),
                  },
                  {
                    text: "Ask about selection",
                    enabled: canAttach,
                    action: () => attachSelection(selected, true),
                  },
                ]
              : []),
            copyAction(
              view === viewer.original
                ? "Copy HEAD version"
                : "Copy file preview",
              view === viewer.original
                ? (latest.current.original ?? "")
                : latest.current.content,
            ),
            copyAction("Copy relative path", path),
          ]);
        }}
      />
    </>
  );
}
