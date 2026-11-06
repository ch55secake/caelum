import { useLayoutEffect, useRef, useState } from "react";

// A quick, non-overshooting settle shared by the workspace panes. Animate the
// slots, not React state, and keep closing contents at their readable width.
const duration = 220;
const easing = "cubic-bezier(.2, .8, .2, 1)";

export function usePaneMotion(open: boolean, size: string, layout: string) {
  const ref = useRef<HTMLDivElement>(null);
  const [retained, setRetained] = useState(open);
  const previous = useRef<{ open: boolean; layout: string } | null>(null);
  const animations = useRef<Animation[]>([]);
  const finish = useRef(() => {});
  const cancel = () => {
    for (const animation of animations.current) {
      animation.onfinish = null;
      animation.cancel();
    }
    animations.current = [];
  };

  useLayoutEffect(() => {
    const node = ref.current!;
    if (open) setRetained(true);
    // Sample before cancelling: reversing a toggle starts where it is on screen.
    const from = node.getBoundingClientRect().width;
    const opacity = getComputedStyle(node).opacity;
    const visible = node.getClientRects().length > 0;
    cancel();
    const before = previous.current;
    previous.current = { open, layout };
    const target = open ? size : "0px";
    delete node.dataset.motion;
    finish.current = () => {
      cancel();
      delete node.dataset.motion;
      setRetained(open);
    };

    // Preferred-width changes (pointer/keyboard resizing) and window resizes
    // stay direct. Only disclosure/layout actions opt into motion.
    if (
      !before ||
      (before.open === open && before.layout === layout) ||
      matchMedia("(prefers-reduced-motion: reduce)").matches ||
      !visible
    ) {
      node.style.flexBasis = target;
      node.style.opacity = open ? "1" : "0";
      finish.current();
      return;
    }

    // Do not measure the target layout here: briefly laying out the wider
    // transcript would clamp its scroll position before the first frame.
    node.style.setProperty("--pane-content-width", `${from}px`);
    node.dataset.motion = open ? "opening" : "closing";
    const width = node.animate(
      [{ flexBasis: `${from}px` }, { flexBasis: target }],
      { duration, easing },
    );
    animations.current = [width];
    if (before.open !== open) {
      animations.current.push(
        node.animate([{ opacity }, { opacity: open ? 1 : 0 }], {
          duration: open ? 110 : 77,
          easing: open ? "ease-out" : "ease-in",
        }),
      );
    }
    width.onfinish = () => finish.current();
    node.style.flexBasis = target;
    node.style.opacity = open ? "1" : "0";
  }, [open, size, layout]);

  useLayoutEffect(() => {
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");
    const change = () => {
      if (reduced.matches) finish.current();
    };
    reduced.addEventListener("change", change);
    return () => {
      reduced.removeEventListener("change", change);
      cancel();
    };
  }, []);

  return { ref, present: open || retained };
}
