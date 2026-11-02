import { useEffect, useRef } from "react";

export function useEscapeCancellation({
  scope,
  enabled,
  cancel,
}: {
  scope: string;
  enabled: boolean;
  cancel: () => void;
}) {
  const action = useRef(cancel);
  action.current = cancel;
  useEffect(() => {
    if (!enabled) return;
    let pressedAt: number | null = null;
    let candidate: { event: KeyboardEvent; previous: number | null } | null = null;
    const reset = () => {
      pressedAt = null;
      candidate = null;
    };
    const capture = (event: KeyboardEvent) => {
      const previous = pressedAt;
      reset();
      if (
        event.key !== "Escape" ||
        event.repeat ||
        event.isComposing ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey ||
        event.shiftKey ||
        document.querySelector('dialog[open], [role="dialog"], [role="menu"]')
      )
        return;
      candidate = { event, previous };
    };
    const keyDown = (event: KeyboardEvent) => {
      if (candidate?.event !== event || event.defaultPrevented) return;
      const now = performance.now();
      const previous = candidate.previous;
      candidate = null;
      if (previous !== null && now - previous <= 750) action.current();
      else pressedAt = now;
    };
    // Capture resets the sequence even when a local dismissal stops bubbling.
    // Only an unhandled Escape reaching the window can arm or cancel a turn.
    window.addEventListener("keydown", capture, true);
    window.addEventListener("keydown", keyDown);
    window.addEventListener("pointerdown", reset, true);
    window.addEventListener("blur", reset);
    return () => {
      window.removeEventListener("keydown", capture, true);
      window.removeEventListener("keydown", keyDown);
      window.removeEventListener("pointerdown", reset, true);
      window.removeEventListener("blur", reset);
    };
  }, [scope, enabled]);
}
