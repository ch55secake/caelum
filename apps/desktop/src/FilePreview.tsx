import { useEffect, useRef, useState } from "react";
import type { editor, IDisposable } from "monaco-editor";
import { Editor, DiffEditor } from "./MonacoEditors.js";
import { languageForPath } from "./codeLanguages.js";
import {
  createFileContext,
  type FileContext,
  type FileLocation,
} from "./fileContext.js";

export default function FilePreview({
  project,
  path,
  content,
  original,
  theme,
  viewState,
  saveView,
  jump,
  attach,
  canAttach,
}: {
  project: string;
  path: string;
  content: string;
  original?: string;
  theme: string;
  viewState: editor.ICodeEditorViewState | null;
  saveView: (view: editor.ICodeEditorViewState | null) => void;
  jump?: FileLocation & { id: number };
  attach: (file: FileContext, ask?: boolean) => void;
  canAttach: boolean;
}) {
  const instance = useRef<editor.IStandaloneCodeEditor | null>(null);
  const resources = useRef<IDisposable[]>([]);
  const previousContent = useRef(content);
  const refreshing = useRef(false);
  const beforeRefresh = useRef(viewState);
  if (previousContent.current !== content) {
    refreshing.current = true;
    beforeRefresh.current = viewState;
    previousContent.current = content;
  }
  const latest = useRef({ saveView, attach, path, viewState, canAttach });
  latest.current = { saveView, attach, path, viewState, canAttach };
  const [selection, setSelection] = useState<string | null>(null);
  const attachSelection = (ask = false) => {
    const model = instance.current?.getModel();
    const range = instance.current?.getSelection();
    if (!latest.current.canAttach || !model || !range || range.isEmpty())
      return;
    const text = model.getValueInRange(range);
    latest.current.attach(
      createFileContext(latest.current.path, text, {
        startLine: range.startLineNumber,
        endLine:
          range.endColumn === 1 && range.endLineNumber > range.startLineNumber
            ? range.endLineNumber - 1
            : range.endLineNumber,
      }),
      ask,
    );
  };
  const reveal = () => {
    if (!jump || !instance.current) return;
    const model = instance.current.getModel();
    const position =
      jump.line &&
      model?.validatePosition({
        lineNumber: jump.line,
        column: jump.column ?? 1,
      });
    if (position) {
      instance.current.setPosition(position);
      instance.current.revealPositionInCenter(position);
    }
    instance.current.focus();
  };
  function mount(code: editor.IStandaloneCodeEditor) {
    instance.current = code;
    try {
      code.restoreViewState(latest.current.viewState);
    } catch {
      /* A changed file may no longer support the saved view. */
    }
    const save = () => {
      if (!refreshing.current) latest.current.saveView(code.saveViewState());
    };
    const showSelection = () => {
      const range = code.getSelection();
      const end =
        range &&
        range.endColumn === 1 &&
        range.endLineNumber > range.startLineNumber
          ? range.endLineNumber - 1
          : range?.endLineNumber;
      setSelection(
        range && !range.isEmpty()
          ? range.startLineNumber === end
            ? `Line ${end}`
            : `Lines ${range.startLineNumber}–${end}`
          : null,
      );
    };
    resources.current.push(
      code.onDidScrollChange(save),
      code.onDidChangeCursorSelection(() => {
        showSelection();
        save();
      }),
      code.addAction({
        id: "caelum.add-selection",
        label: "Add selection to prompt",
        contextMenuGroupId: "9_caelum",
        precondition: "editorHasSelection",
        run: () => attachSelection(),
      }),
      code.addAction({
        id: "caelum.ask-selection",
        label: "Ask about selection",
        contextMenuGroupId: "9_caelum",
        precondition: "editorHasSelection",
        run: () => attachSelection(true),
      }),
    );
    showSelection();
    reveal();
  }
  useEffect(() => reveal(), [jump?.id]);
  useEffect(() => {
    // A live refresh changes the model's text, but preserves the reader's place.
    if (!refreshing.current) return;
    try {
      instance.current?.restoreViewState(beforeRefresh.current);
    } catch {
      /* Use the editor's clamped position. */
    }
    refreshing.current = false;
    if (instance.current)
      latest.current.saveView(instance.current.saveViewState());
  }, [content]);
  useEffect(
    () => () => {
      resources.current.forEach((resource) => resource.dispose());
    },
    [],
  );
  const options = {
    ariaLabel: `Read-only ${original === undefined ? "preview of" : "diff for"} ${path}`,
    automaticLayout: true,
    fontSize: 13,
    lineNumbers: "on" as const,
    minimap: { enabled: false },
    readOnly: true,
    scrollBeyondLastLine: false,
    wordWrap: "off" as const,
  };
  return (
    <>
      {selection && (
        <div className="file-selection-actions">
          <span>{selection}</span>
          <button disabled={!canAttach} onClick={() => attachSelection()}>
            Add selection to prompt
          </button>
          <button disabled={!canAttach} onClick={() => attachSelection(true)}>
            Ask about selection
          </button>
        </div>
      )}
      <div className="monaco-container">
        {original === undefined ? (
          <Editor
            height="100%"
            path={`${project}/${path}`}
            theme={theme}
            language={languageForPath(path)}
            value={content}
            options={options}
            onMount={mount}
          />
        ) : (
          <DiffEditor
            height="100%"
            originalModelPath={`${project}/${path}?base`}
            modifiedModelPath={`${project}/${path}?diff`}
            theme={theme}
            language={languageForPath(path)}
            original={original}
            modified={content}
            options={{
              ...options,
              renderSideBySide: true,
              originalEditable: false,
            }}
            onMount={(diff) => mount(diff.getModifiedEditor())}
          />
        )}
      </div>
    </>
  );
}
