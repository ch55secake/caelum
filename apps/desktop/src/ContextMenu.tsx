import {
  useCallback,
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { copyText } from "./clipboard.js";
import { editingActions } from "./contextMenuEditing.js";

export type ContextAction =
  | { text: string; enabled?: boolean; action: () => void | Promise<unknown> }
  | { item: "Separator" };
type ContextEvent = Pick<
  MouseEvent,
  "target" | "clientX" | "clientY" | "preventDefault" | "stopPropagation"
>;
type OpenMenu = (event: ContextEvent, actions: ContextAction[]) => void;
const ContextMenus = createContext<OpenMenu>(() => {});
export const useContextMenu = () => useContext(ContextMenus);
const separator: ContextAction = { item: "Separator" };

export const copyAction = (text: string, value: string): ContextAction => ({
  text,
  action: () => copyText(value),
});
export const revealAction = (path: string): ContextAction => ({
  text: /Macintosh|Mac OS X/.test(navigator.userAgent)
    ? "Reveal in Finder"
    : "Reveal in file manager",
  action: () => revealItemInDir(path),
});
export function fileActions(
  project: string,
  path: string,
  actions: ContextAction[] = [],
): ContextAction[] {
  const absolute = `${project.replace(/[\\/]$/, "")}/${path}`;
  return [
    ...actions,
    ...(actions.length ? [separator] : []),
    copyAction("Copy relative path", path),
    copyAction("Copy full path", absolute),
    revealAction(absolute),
  ];
}

function contextualActions(
  target: Element,
  actions: ContextAction[],
  current: () => boolean,
): { items: ContextAction[]; restoreFocus?: () => void } {
  const editing = editingActions(target, current);
  if (editing) return editing;
  if (target.closest(".code-viewer")) return { items: actions };
  const selection = window.getSelection();
  const selected =
    selection && !selection.isCollapsed && selection.containsNode(target, true)
      ? selection.toString()
      : "";
  const link = target.closest<HTMLAnchorElement>("a[href]");
  const common: ContextAction[] = [
    ...(selected ? [copyAction("Copy selection", selected)] : []),
    ...(link
      ? [
          {
            text: "Open link",
            enabled: /^(https?:|mailto:)/i.test(link.href),
            action: () => openUrl(link.href),
          },
          copyAction("Copy link address", link.href),
        ]
      : []),
  ];
  return {
    items: [
      ...common,
      ...(common.length && actions.length ? [separator] : []),
      ...actions,
    ],
  };
}

type OpenedMenu = {
  items: ContextAction[];
  x: number;
  y: number;
  target: Element;
  restoreFocus: () => void;
  version: number;
};

function ContextMenu({
  menu,
  close,
  choose,
}: {
  menu: OpenedMenu;
  close: (restore?: boolean) => void;
  choose: (menu: OpenedMenu, item: ContextAction) => void;
}) {
  const popup = useRef<HTMLDivElement>(null);
  const search = useRef({ text: "", time: 0 });
  const buttons = () =>
    Array.from(
      popup.current!.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    );
  useLayoutEffect(() => {
    const element = popup.current!;
    const rect = element.getBoundingClientRect();
    element.style.left = `${Math.max(8, Math.min(menu.x, window.innerWidth - rect.width - 8))}px`;
    element.style.top = `${Math.max(8, Math.min(menu.y, window.innerHeight - rect.height - 8))}px`;
    (buttons()[0] ?? element).focus({ preventScroll: true });
  }, [menu]);
  useEffect(() => {
    const outside = (event: Event) => {
      if (!popup.current?.contains(event.target as Node)) close();
    };
    const dismiss = () => close();
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    // Editor focus/selection can scroll asynchronously after keyboard invocation.
    // Dismiss for user scrolling, not those pending layout adjustments.
    document.addEventListener("wheel", outside, { passive: true });
    document.addEventListener("touchmove", outside, { passive: true });
    window.addEventListener("resize", dismiss);
    window.addEventListener("blur", dismiss);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("wheel", outside);
      document.removeEventListener("touchmove", outside);
      window.removeEventListener("resize", dismiss);
      window.removeEventListener("blur", dismiss);
    };
  }, [close]);
  return createPortal(
    <div
      ref={popup}
      className="context-menu"
      role="menu"
      aria-label="Context menu"
      tabIndex={-1}
      onContextMenu={(event) => {
        event.preventDefault();
        event.stopPropagation();
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape" || event.key === "Tab") {
          event.preventDefault();
          event.stopPropagation();
          close(true);
          return;
        }
        if (event.altKey || event.ctrlKey || event.metaKey) return;
        const items = buttons();
        const at = items.indexOf(document.activeElement as HTMLButtonElement);
        let next: HTMLButtonElement | undefined;
        if (event.key === "ArrowDown" || event.key === "ArrowUp")
          next =
            items[
              (at + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
                items.length
            ];
        else if (event.key === "Home") next = items[0];
        else if (event.key === "End") next = items.at(-1);
        else if (event.key.length === 1 && event.key !== " ") {
          const now = performance.now();
          const letter = event.key.toLowerCase();
          const text =
            now - search.current.time < 700
              ? search.current.text + letter
              : letter;
          search.current = { text, time: now };
          const ordered = [...items.slice(at + 1), ...items.slice(0, at + 1)];
          next =
            ordered.find((item) =>
              item.textContent?.toLowerCase().startsWith(text),
            ) ??
            ordered.find((item) =>
              item.textContent?.toLowerCase().startsWith(letter),
            );
        } else return;
        event.preventDefault();
        event.stopPropagation();
        next?.focus({ preventScroll: true });
        next?.scrollIntoView({ block: "nearest" });
      }}
    >
      {menu.items.map((item, index) =>
        "item" in item ? (
          <div
            key={index}
            className="context-menu-separator"
            role="separator"
          />
        ) : (
          <button
            key={index}
            type="button"
            role="menuitem"
            tabIndex={-1}
            disabled={item.enabled === false}
            onPointerMove={(event) => {
              if (event.pointerType === "mouse")
                event.currentTarget.focus({ preventScroll: true });
            }}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => choose(menu, item)}
          >
            {item.text}
          </button>
        ),
      )}
    </div>,
    // A menu in a modal must belong to its top-layer dialog to stay interactive.
    menu.target.closest('dialog[open], [role="dialog"]') ?? document.body,
  );
}

export function ContextMenuProvider({
  children,
  className,
  actions,
  scope,
  reportError,
}: {
  children: ReactNode;
  className: string;
  actions: ContextAction[];
  scope: string;
  reportError: (message: string) => void;
}) {
  const [menu, setMenu] = useState<OpenedMenu | null>(null);
  const active = useRef<OpenedMenu | null>(null);
  const revision = useRef(0);
  const options = useRef({ actions, reportError });
  options.current = { actions, reportError };
  const release = useCallback((restore = false) => {
    revision.current++;
    const previous = active.current;
    active.current = null;
    setMenu(null);
    if (restore) previous?.restoreFocus();
  }, []);
  const dismiss = useCallback(
    (restore = false) => {
      if (active.current) release(restore);
    },
    [release],
  );
  useLayoutEffect(() => {
    release();
    return () => {
      revision.current++;
      active.current = null;
    };
  }, [scope, release]);
  const open: OpenMenu = (event, actions) => {
    if (!(event.target instanceof Element)) return;
    event.preventDefault();
    event.stopPropagation();
    release();
    const version = revision.current;
    const context = contextualActions(
      event.target,
      actions,
      () => revision.current === version,
    );
    if (!context.items.length) return;
    const trigger =
      event.target.closest<HTMLElement>(
        "button, a[href], input, textarea, select, [tabindex]",
      ) ?? document.activeElement;
    const next = {
      items: context.items,
      x: event.clientX ?? 0,
      y: event.clientY ?? 0,
      target: event.target,
      restoreFocus:
        context.restoreFocus ??
        (() => {
          if (trigger instanceof HTMLElement && trigger.isConnected)
            trigger.focus({ preventScroll: true });
        }),
      version,
    };
    active.current = next;
    setMenu(next);
  };
  const choose = (menu: OpenedMenu, item: ContextAction) => {
    if (
      active.current !== menu ||
      revision.current !== menu.version ||
      !menu.target.isConnected ||
      "item" in item ||
      item.enabled === false
    )
      return;
    active.current = null;
    setMenu(null);
    menu.restoreFocus();
    try {
      void Promise.resolve(item.action()).catch((reason) => {
        if (revision.current === menu.version)
          options.current.reportError(String(reason));
      });
    } catch (reason) {
      options.current.reportError(String(reason));
    }
  };
  const show = useRef(open);
  show.current = open;
  useEffect(() => {
    const context = (event: MouseEvent) => {
      if (event.defaultPrevented) return;
      const inDialog =
        event.target instanceof Element &&
        event.target.closest('dialog, [role="dialog"]');
      show.current(event, inDialog ? [] : options.current.actions);
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.defaultPrevented || !(event.target instanceof HTMLElement))
        return;
      if (
        event.key !== "ContextMenu" &&
        !(event.shiftKey && event.key === "F10")
      )
        return;
      event.preventDefault();
      const rect = event.target.getBoundingClientRect();
      event.target.dispatchEvent(
        new MouseEvent("contextmenu", {
          bubbles: true,
          cancelable: true,
          clientX: rect.left + 8,
          clientY: rect.bottom,
        }),
      );
    };
    document.addEventListener("contextmenu", context);
    document.addEventListener("keydown", keyboard);
    return () => {
      document.removeEventListener("contextmenu", context);
      document.removeEventListener("keydown", keyboard);
    };
  }, []);
  return (
    <ContextMenus.Provider value={open}>
      <main className={className}>{children}</main>
      {menu && (
        <ContextMenu
          key={menu.version}
          menu={menu}
          close={dismiss}
          choose={choose}
        />
      )}
    </ContextMenus.Provider>
  );
}
