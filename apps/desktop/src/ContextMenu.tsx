import {
  createContext,
  useContext,
  useEffect,
  useLayoutEffect,
  useRef,
  type ReactNode,
} from "react";
import { isTauri } from "@tauri-apps/api/core";
import { LogicalPosition } from "@tauri-apps/api/dpi";
import { Menu, type PredefinedMenuItemOptions } from "@tauri-apps/api/menu";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { copyText } from "./clipboard.js";

export type ContextAction =
  | { text: string; enabled?: boolean; action: () => void | Promise<unknown> }
  | PredefinedMenuItemOptions;
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
): ContextAction[] {
  const input = target.closest<HTMLInputElement | HTMLTextAreaElement>(
    "input, textarea",
  );
  const field =
    input instanceof HTMLTextAreaElement ||
    (input instanceof HTMLInputElement &&
      ["text", "search", "url", "email", "tel", "number", "password"].includes(
        input.type,
      ))
      ? input
      : null;
  const editableElement = target.closest<HTMLElement>(
    '[contenteditable="true"]',
  );
  if (field || editableElement) {
    if (field?.disabled) return [];
    (field ?? editableElement)!.focus({ preventScroll: true });
    const editable = !field?.readOnly;
    const secret =
      field instanceof HTMLInputElement && field.type === "password";
    return [
      ...(editable
        ? ([{ item: "Undo" }, { item: "Redo" }, separator] as ContextAction[])
        : []),
      ...(editable && !secret ? ([{ item: "Cut" }] as ContextAction[]) : []),
      ...(!secret ? ([{ item: "Copy" }] as ContextAction[]) : []),
      ...(editable ? ([{ item: "Paste" }] as ContextAction[]) : []),
      separator,
      { item: "SelectAll" },
    ];
  }
  if (target.closest(".code-viewer")) return actions;
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
  return [
    ...common,
    ...(common.length && actions.length ? [separator] : []),
    ...actions,
  ];
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
  const active = useRef<Menu | null>(null);
  const revision = useRef(0);
  const options = useRef({ actions, reportError });
  options.current = { actions, reportError };
  const release = () => {
    revision.current++;
    const previous = active.current;
    active.current = null;
    void previous?.close().catch(() => {});
  };
  useLayoutEffect(() => release, [scope]);
  const open: OpenMenu = (event, actions) => {
    if (!isTauri() || !(event.target instanceof Element)) return;
    event.preventDefault();
    event.stopPropagation();
    const items = contextualActions(event.target, actions);
    release();
    if (!items.length) return;
    const version = revision.current;
    const at = new LogicalPosition(event.clientX, event.clientY);
    let used = false;
    void Menu.new({
      items: items.map((item) =>
        "item" in item
          ? item
          : {
              text: item.text,
              enabled: item.enabled,
              action: () => {
                if (
                  used ||
                  revision.current !== version ||
                  item.enabled === false
                )
                  return;
                used = true;
                void Promise.resolve()
                  .then(item.action)
                  .catch((reason) =>
                    options.current.reportError(String(reason)),
                  );
              },
            },
      ),
    })
      .then(async (menu) => {
        if (revision.current !== version) return menu.close();
        // popup resolves on display, so retain the resource until the next menu/navigation.
        active.current = menu;
        await menu.popup(at);
      })
      .catch((reason) => {
        if (revision.current === version) {
          release();
          options.current.reportError(
            `Could not open context menu: ${String(reason)}`,
          );
        }
      });
  };
  const show = useRef(open);
  show.current = open;
  useEffect(() => {
    const context = (event: MouseEvent) => {
      if (event.defaultPrevented) return;
      const inDialog =
        event.target instanceof Element &&
        event.target.closest('[role="dialog"]');
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
    </ContextMenus.Provider>
  );
}
