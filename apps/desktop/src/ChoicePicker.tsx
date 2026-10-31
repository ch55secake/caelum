import {
  Fragment,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, Search, Star } from "lucide-react";

type Choice = {
  id: string;
  name: string;
  description?: string;
  source?: string;
  group?: { id: string; name: string };
};

export function ChoicePicker({
  label,
  value,
  choices,
  onChange,
  icon,
  disabled,
  triggerName,
  searchable = false,
  favouriteIds = [],
  onToggleFavourite,
}: {
  label: string;
  value: string;
  choices: Choice[];
  onChange: (value: string) => void;
  icon: ReactNode;
  disabled?: boolean;
  triggerName?: string;
  searchable?: boolean;
  favouriteIds?: string[];
  onToggleFavourite?: (id: string) => void;
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState<string | null>(null);
  const [position, setPosition] = useState<{
    left: number;
    top?: number;
    bottom?: number;
    width: number;
    maxHeight: number;
  }>({
    left: 0,
    bottom: 0,
    width: 320,
    maxHeight: 360,
  });
  const favourites = new Set(favouriteIds);
  const rank = (choice: Choice) =>
    !choice.id ? 0 : favourites.has(choice.id) ? 1 : 2;
  const ordered = onToggleFavourite
    ? [...choices].sort(
        (a, b) =>
          rank(a) - rank(b) ||
          (rank(a) === 2
            ? (a.group?.name ?? "").localeCompare(b.group?.name ?? "") ||
              (a.group?.id ?? "").localeCompare(b.group?.id ?? "")
            : 0),
      )
    : choices;
  const filtered = ordered.filter((choice) =>
    `${choice.name} ${choice.id} ${choice.source ?? ""} ${choice.group?.name ?? ""}`
      .toLowerCase()
      .includes(query.toLowerCase()),
  );
  const selected = choices.find((choice) => choice.id === value);
  const current =
    filtered.find((choice) => choice.id === active) ?? filtered[0];
  const activeIndex = filtered.indexOf(current);
  const activeId = current ? `${id}-option-${activeIndex}` : undefined;
  const section = (choice: Choice) =>
    !choice.id
      ? ""
      : favourites.has(choice.id)
        ? "Favourites"
        : (choice.group?.name ?? "All models");
  const sectionKey = (choice: Choice) =>
    !choice.id
      ? "default"
      : favourites.has(choice.id)
        ? "favourites"
        : `provider:${choice.group?.id ?? ""}`;

  function show() {
    setQuery("");
    setActive(value);
    setOpen(true);
  }
  function close(restoreFocus = false) {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  }
  function choose(choice: Choice) {
    onChange(choice.id);
    close(true);
  }

  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const rect = trigger.current!.getBoundingClientRect();
      const width = Math.min(searchable ? 340 : 240, window.innerWidth - 24);
      const below =
        rect.top < 300 && window.innerHeight - rect.bottom > rect.top;
      setPosition({
        width,
        left: Math.max(
          12,
          Math.min(rect.left, window.innerWidth - width - 12),
        ),
        ...(below
          ? { top: rect.bottom + 8 }
          : { bottom: window.innerHeight - rect.top + 8 }),
        maxHeight: Math.min(
          360,
          below ? window.innerHeight - rect.bottom - 20 : rect.top - 20,
        ),
      });
    };
    place();
    (search.current ?? list.current)?.focus();
    window.addEventListener("resize", place);
    return () => window.removeEventListener("resize", place);
  }, [open, searchable]);

  useEffect(() => {
    if (!open) return;
    const outside = (event: PointerEvent) => {
      if (
        !popup.current?.contains(event.target as Node) &&
        !trigger.current?.contains(event.target as Node)
      )
        close();
    };
    const scrolled = (event: Event) => {
      if (!popup.current?.contains(event.target as Node)) close();
    };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("scroll", scrolled, true);
    return () => {
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("scroll", scrolled, true);
    };
  }, [open]);
  useEffect(() => {
    if (open && activeId)
      document.getElementById(activeId)?.scrollIntoView({ block: "nearest" });
  }, [open, activeId]);
  useEffect(() => {
    if (disabled) setOpen(false);
  }, [disabled]);

  return (
    <>
      <button
        ref={trigger}
        type="button"
        className="choice-trigger"
        aria-label={label}
        title={`${label}: ${selected?.name ?? value}${selected?.source ? ` · ${selected.source} (${selected.id})` : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? `${id}-popup` : undefined}
        disabled={disabled}
        onClick={() => (open ? close() : show())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            show();
          }
        }}
      >
        {icon}
        <span className="choice-trigger-name">
          {triggerName ?? selected?.name ?? value}
        </span>
        {selected?.source && (
          <small className="choice-trigger-source">{selected.source}</small>
        )}
        <ChevronDown size={11} />
      </button>
      {open &&
        createPortal(
          <div
            ref={popup}
            id={`${id}-popup`}
            className="choice-popover"
            style={position}
            role="dialog"
            aria-label={`Choose ${label.toLowerCase()}`}
            onBlur={(event) => {
              if (
                !event.currentTarget.contains(event.relatedTarget as Node) &&
                event.relatedTarget !== trigger.current
              )
                close();
            }}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                event.preventDefault();
                event.stopPropagation();
                close(true);
              }
              if (event.key === "ArrowDown" || event.key === "ArrowUp") {
                event.preventDefault();
                setActive(
                  filtered.length
                    ? filtered[
                        (activeIndex +
                          (event.key === "ArrowDown" ? 1 : -1) +
                          filtered.length) %
                          filtered.length
                      ].id
                    : null,
                );
                (search.current ?? list.current)?.focus();
              }
              if (
                event.key === "Enter" &&
                !event.nativeEvent.isComposing &&
                !(event.target as HTMLElement).closest(".choice-favourite")
              ) {
                event.preventDefault();
                event.stopPropagation();
                if (current) choose(current);
              }
            }}
          >
            {searchable ? (
              <label className="choice-search">
                <Search size={14} />
                <input
                  ref={search}
                  role="combobox"
                  aria-label={`Search ${label.toLowerCase()}s`}
                  aria-expanded="true"
                  aria-controls={`${id}-list`}
                  aria-autocomplete="list"
                  aria-haspopup={onToggleFavourite ? "grid" : "listbox"}
                  aria-activedescendant={activeId}
                  placeholder={`Search ${label.toLowerCase()}s…`}
                  value={query}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setActive(null);
                  }}
                />
              </label>
            ) : (
              <div className="choice-heading">{label}</div>
            )}
            <div
              ref={list}
              id={`${id}-list`}
              role={onToggleFavourite ? "grid" : "listbox"}
              aria-label={label}
              aria-activedescendant={!searchable ? activeId : undefined}
              tabIndex={searchable ? -1 : 0}
              className="choice-options"
            >
              {filtered.map((choice, index) => {
                const optionId = `${id}-option-${index}`;
                const button = (
                  <button
                    key={choice.id}
                    id={onToggleFavourite ? undefined : optionId}
                    type="button"
                    role={onToggleFavourite ? undefined : "option"}
                    tabIndex={-1}
                    aria-selected={
                      onToggleFavourite ? undefined : choice.id === value
                    }
                    aria-label={
                      onToggleFavourite
                        ? `${choice.name}${choice.description ? ` ${choice.description}` : ""}`
                        : undefined
                    }
                    aria-description={
                      choice.source ? `Provider: ${choice.source}` : undefined
                    }
                    className={`choice-option${current?.id === choice.id ? " highlighted" : ""}`}
                    onMouseDown={(event) => event.preventDefault()}
                    onFocus={() => setActive(choice.id)}
                    onClick={() => choose(choice)}
                  >
                    <span>
                      <span className="choice-option-name">
                        <strong>{choice.name}</strong>
                        {choice.source && (
                          <small className="choice-source">
                            {choice.source}
                          </small>
                        )}
                      </span>
                      {choice.description && (
                        <small>{choice.description}</small>
                      )}
                    </span>
                    {choice.id === value && <Check size={14} />}
                  </button>
                );
                if (!onToggleFavourite) return button;
                const favourite = favourites.has(choice.id);
                const favouriteLabel = `${favourite ? "Remove" : "Add"} ${choice.name} (${choice.id}) ${favourite ? "from" : "to"} favourites`;
                return (
                  <Fragment key={choice.id}>
                    {section(choice) &&
                      (index === 0 ||
                        sectionKey(choice) !==
                          sectionKey(filtered[index - 1])) && (
                        <div role="row" className="choice-section">
                          <div role="gridcell" aria-colspan={2}>
                            {section(choice)}
                          </div>
                        </div>
                      )}
                    <div
                      role="row"
                      aria-selected={choice.id === value}
                      className={`choice-model-row${current?.id === choice.id ? " highlighted" : ""}`}
                    >
                      <div role="gridcell" id={optionId}>
                        {button}
                      </div>
                      <div role="gridcell">
                        {choice.id && (
                          <button
                            type="button"
                            className="choice-favourite"
                            aria-label={favouriteLabel}
                            title={favouriteLabel}
                            aria-pressed={favourite}
                            tabIndex={current?.id === choice.id ? 0 : -1}
                            onFocus={() => setActive(choice.id)}
                            onMouseDown={(event) => event.preventDefault()}
                            onClick={() => {
                              setActive(choice.id);
                              onToggleFavourite(choice.id);
                              (search.current ?? list.current)?.focus();
                            }}
                          >
                            <Star
                              size={14}
                              fill={favourite ? "currentColor" : "none"}
                            />
                          </button>
                        )}
                      </div>
                    </div>
                  </Fragment>
                );
              })}
              {!filtered.length && (
                <div className="choice-empty">
                  No matching {label.toLowerCase()}s
                </div>
              )}
            </div>
          </div>,
          trigger.current?.closest("dialog") ?? document.body,
        )}
    </>
  );
}
