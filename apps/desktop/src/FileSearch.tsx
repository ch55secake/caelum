import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Search, X } from "lucide-react";
import type { FileLocation } from "./fileContext.js";

export type FileSearchRequest = { id: number; mode: "files" | "content" };
type SearchMatch = FileLocation & { preview: string | null };
type SearchResults = {
  matches: SearchMatch[];
  truncated: boolean;
  skipped: number;
};

export function FileSearch({
  project,
  request,
  openFile,
}: {
  project: string;
  request?: FileSearchRequest;
  openFile: (location: FileLocation) => void;
}) {
  const [mode, setMode] = useState<"files" | "content">("files");
  const [query, setQuery] = useState("");
  const [includeIgnored, setIncludeIgnored] = useState(false);
  const [open, setOpen] = useState(false);
  const [result, setResult] = useState<SearchResults | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [active, setActive] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const container = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!request) return;
    setMode(request.mode);
    setOpen(true);
    input.current?.focus();
    input.current?.select();
  }, [request?.id]);
  useEffect(() => {
    if (!open) return;
    let disposed = false;
    setLoading(true);
    setError(null);
    setResult(null);
    setActive(0);
    const timer = window.setTimeout(() => {
      void invoke<SearchResults>("search_project_files", {
        projectPath: project,
        query,
        mode,
        includeIgnored,
      })
        .then(
          (value) => {
            if (!disposed) setResult(value);
          },
          (reason) => {
            if (!disposed) setError(String(reason));
          },
        )
        .finally(() => {
          if (!disposed) setLoading(false);
        });
    }, 160);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [project, query, mode, includeIgnored, open]);
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [active]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (!container.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [open]);
  function choose(match: SearchMatch) {
    setOpen(false);
    openFile(match);
  }
  return (
    <div className="file-search" ref={container}>
      <div className="file-search-input">
        <Search size={14} />
        <input
          ref={input}
          type="text"
          role="combobox"
          aria-label={
            mode === "files" ? "Find project file" : "Search project contents"
          }
          aria-expanded={open}
          aria-controls="file-search-results"
          aria-autocomplete="list"
          aria-activedescendant={
            open && result?.matches[active]
              ? `file-search-${active}`
              : undefined
          }
          placeholder={
            mode === "files" ? "Find file…  ⌘P" : "Search contents…  ⌘⇧F"
          }
          maxLength={256}
          value={query}
          onFocus={() => setOpen(true)}
          onChange={(event) => {
            setQuery(event.target.value);
            setOpen(true);
          }}
          onKeyDown={(event) => {
            if (event.nativeEvent.isComposing) return;
            if (event.key === "Escape") {
              event.preventDefault();
              event.stopPropagation();
              setOpen(false);
              input.current?.blur();
            }
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
              event.preventDefault();
              setOpen(true);
              setActive((index) =>
                Math.max(
                  0,
                  Math.min(
                    (result?.matches.length ?? 1) - 1,
                    index + (event.key === "ArrowDown" ? 1 : -1),
                  ),
                ),
              );
            }
            if (event.key === "Enter") {
              event.preventDefault();
              if (result?.matches[active]) choose(result.matches[active]);
            }
          }}
        />
        {query && (
          <button
            className="icon-button"
            aria-label="Clear file search"
            onClick={() => {
              setQuery("");
              input.current?.focus();
            }}
          >
            <X size={12} />
          </button>
        )}
      </div>
      {open && (
        <div className="file-search-popup">
          <div className="file-search-options">
            <div role="group" aria-label="File search mode">
              <button
                aria-pressed={mode === "files"}
                onClick={() => {
                  setMode("files");
                  input.current?.focus();
                }}
              >
                Filename
              </button>
              <button
                aria-pressed={mode === "content"}
                onClick={() => {
                  setMode("content");
                  input.current?.focus();
                }}
              >
                Contents
              </button>
            </div>
            <label>
              <input
                type="checkbox"
                checked={includeIgnored}
                onChange={(event) => setIncludeIgnored(event.target.checked)}
              />
              Include ignored
            </label>
          </div>
          {loading && (
            <div className="file-search-note" role="status">
              Searching…
            </div>
          )}
          {error && (
            <div className="file-search-note" role="alert">
              {error}
            </div>
          )}
          <div
            id="file-search-results"
            className="file-search-results"
            role="listbox"
            aria-label="Project search results"
            ref={list}
          >
            {result?.matches.map((match, index) => (
              <button
                key={`${match.path}:${match.line ?? 0}`}
                id={`file-search-${index}`}
                role="option"
                aria-selected={index === active}
                onMouseEnter={() => setActive(index)}
                onClick={() => choose(match)}
              >
                <span>
                  {match.path}
                  {match.line ? `:${match.line}` : ""}
                </span>
                {match.preview !== null && <code>{match.preview}</code>}
              </button>
            ))}
          </div>
          {result && !result.matches.length && (
            <div className="file-search-note">
              {mode === "content" && !query.trim()
                ? "Type text to search inside files."
                : "No matching files."}
            </div>
          )}
          {result && (result.truncated || result.skipped > 0) && (
            <div className="file-search-note" role="status">
              {result.truncated
                ? "Search limit reached. Narrow your query for more results."
                : ""}{" "}
              {result.skipped > 0
                ? `${result.skipped} unreadable entries skipped.`
                : ""}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
