import { useLayoutEffect, useRef, type RefObject } from "react";

// Read state follows what the user can actually see, independently of session
// selection. Hidden PR/inspector views, modal dialogs and older windows do not
// acknowledge content. The activity controller also gates on window focus.
export function useLatestView(
  scroll: RefObject<HTMLDivElement | null>,
  ready: boolean,
  report: (visible: boolean) => void,
) {
  const latest = useRef({ ready, report });
  latest.current = { ready, report };
  function update() {
    const node = scroll.current;
    latest.current.report(
      Boolean(
        latest.current.ready &&
          node &&
          node.clientHeight > 0 &&
          !node.closest("[hidden], [inert]") &&
          !document.querySelector("dialog[open]") &&
          node.scrollHeight - node.scrollTop - node.clientHeight < 64,
      ),
    );
  }
  useLayoutEffect(update);
  useLayoutEffect(() => {
    const node = scroll.current!;
    const resize = new ResizeObserver(update);
    resize.observe(node);
    const visibility = new MutationObserver(update);
    visibility.observe(document.body, {
      subtree: true,
      attributes: true,
      attributeFilter: ["hidden", "inert", "open"],
      childList: true,
    });
    node.addEventListener("scroll", update);
    return () => {
      resize.disconnect();
      visibility.disconnect();
      node.removeEventListener("scroll", update);
      latest.current.report(false);
    };
  }, []);
}
