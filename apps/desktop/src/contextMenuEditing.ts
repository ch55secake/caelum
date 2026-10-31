import type { ContextAction } from "./ContextMenu.js";
import { copyText, pasteText } from "./clipboard.js";

// Keep browser editing history and React's input events when using app-owned menus.
export function editingActions(
  target: Element,
  current: () => boolean,
): { items: ContextAction[]; restoreFocus?: () => void } | null {
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
  const element =
    field ??
    target.closest<HTMLElement>(
      '[contenteditable="true"], [contenteditable="plaintext-only"]',
    );
  if (!element || (!field && !(target as HTMLElement).isContentEditable))
    return null;
  if (field?.disabled) return { items: [] };
  element.focus({ preventScroll: true });
  const start = field?.selectionStart;
  const end = field?.selectionEnd;
  const direction = field?.selectionDirection;
  const selection = window.getSelection();
  const range =
    !field &&
    selection?.rangeCount &&
    element.contains(selection.getRangeAt(0).commonAncestorContainer)
      ? selection.getRangeAt(0).cloneRange()
      : null;
  const value = field ? field.value : element.innerHTML;
  const selected =
    field && start != null && end != null
      ? value.slice(start, end)
      : (selection?.toString() ?? "");
  const editable =
    !field?.readOnly && element.getAttribute("aria-readonly") !== "true";
  const secret = field instanceof HTMLInputElement && field.type === "password";
  const restore = () => {
    if (!element.isConnected) return false;
    element.focus({ preventScroll: true });
    if (field && start != null && end != null)
      field.setSelectionRange(start, end, direction ?? undefined);
    else if (range && selection) {
      selection.removeAllRanges();
      selection.addRange(range);
    }
    return true;
  };
  const unchanged = () => {
    if (
      !current() ||
      !element.isConnected ||
      document.activeElement !== element
    )
      return false;
    if (field)
      return (
        !field.disabled &&
        !field.readOnly &&
        field.value === value &&
        field.selectionStart === start &&
        field.selectionEnd === end
      );
    const now = window.getSelection();
    const at = now?.rangeCount ? now.getRangeAt(0) : null;
    return (
      element.isContentEditable &&
      element.innerHTML === value &&
      at?.startContainer === range?.startContainer &&
      at?.startOffset === range?.startOffset &&
      at?.endContainer === range?.endContainer &&
      at?.endOffset === range?.endOffset
    );
  };
  const command = (name: string) => () => {
    if (current() && restore()) document.execCommand(name);
  };
  const separator: ContextAction = { item: "Separator" };
  const items: ContextAction[] = [
    ...(editable
      ? [
          {
            text: "Undo",
            enabled: document.queryCommandEnabled("undo"),
            action: command("undo"),
          },
          {
            text: "Redo",
            enabled: document.queryCommandEnabled("redo"),
            action: command("redo"),
          },
          separator,
        ]
      : []),
    ...(editable && !secret
      ? [
          {
            text: "Cut",
            enabled: Boolean(selected),
            action: async () => {
              if (!current() || !restore()) return;
              await copyText(selected);
              if (unchanged()) document.execCommand("delete");
            },
          },
        ]
      : []),
    ...(!secret
      ? [
          {
            text: "Copy",
            enabled: Boolean(selected),
            action: () => copyText(selected),
          },
        ]
      : []),
    ...(editable
      ? [
          {
            text: "Paste",
            action: async () => {
              if (!current() || !restore()) return;
              const text = await pasteText();
              if (unchanged()) document.execCommand("insertText", false, text);
            },
          },
        ]
      : []),
    separator,
    {
      text: "Select all",
      action: () => {
        if (!current() || !restore()) return;
        if (field) field.select();
        else {
          const all = document.createRange();
          all.selectNodeContents(element);
          selection?.removeAllRanges();
          selection?.addRange(all);
        }
      },
    },
  ];
  return { items, restoreFocus: restore };
}
