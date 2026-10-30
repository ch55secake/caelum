import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { Check, ChevronDown, Search } from "lucide-react";

type Choice = { id: string; name: string; description?: string };

export function ChoicePicker({
  label,
  value,
  choices,
  onChange,
  icon,
  disabled,
  searchable = false,
}: {
  label: string;
  value: string;
  choices: Choice[];
  onChange: (value: string) => void;
  icon: ReactNode;
  disabled?: boolean;
  searchable?: boolean;
}) {
  const id = useId();
  const trigger = useRef<HTMLButtonElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
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
  const filtered = choices.filter((choice) =>
    `${choice.name} ${choice.id}`.toLowerCase().includes(query.toLowerCase()),
  );
  const selected = choices.find((choice) => choice.id === value);
  const activeIndex = Math.min(active, filtered.length - 1);
  const current = filtered[activeIndex];
  const activeId = current ? `${id}-option-${activeIndex}` : undefined;

  function show() {
    setQuery("");
    setActive(
      Math.max(
        0,
        choices.findIndex((choice) => choice.id === value),
      ),
    );
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
        title={`${label}: ${selected?.name ?? value}`}
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
        <span>{selected?.name ?? value}</span>
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
                setActive((index) =>
                  filtered.length
                    ? (index +
                        (event.key === "ArrowDown" ? 1 : -1) +
                        filtered.length) %
                      filtered.length
                    : 0,
                );
              }
              if (event.key === "Enter" && !event.nativeEvent.isComposing) {
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
                  aria-activedescendant={activeId}
                  placeholder={`Search ${label.toLowerCase()}s…`}
                  value={query}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setActive(0);
                  }}
                />
              </label>
            ) : (
              <div className="choice-heading">{label}</div>
            )}
            <div
              ref={list}
              id={`${id}-list`}
              role="listbox"
              aria-label={label}
              aria-activedescendant={!searchable ? activeId : undefined}
              tabIndex={searchable ? -1 : 0}
              className="choice-options"
            >
              {filtered.map((choice, index) => (
                <button
                  key={choice.id}
                  id={`${id}-option-${index}`}
                  type="button"
                  role="option"
                  tabIndex={-1}
                  aria-selected={choice.id === value}
                  className={`choice-option${current?.id === choice.id ? " highlighted" : ""}`}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => choose(choice)}
                >
                  <span>
                    <strong>{choice.name}</strong>
                    {choice.description && (
                      <small>{choice.description}</small>
                    )}
                  </span>
                  {choice.id === value && <Check size={14} />}
                </button>
              ))}
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
