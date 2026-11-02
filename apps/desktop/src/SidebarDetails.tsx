import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

export function SidebarDetails({
  id,
  className,
  details,
  children,
}: {
  id: string;
  className: string;
  details: ReactNode;
  children: ReactNode;
}) {
  const anchor = useRef<HTMLDivElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const visible = (hovered || focused) && !dismissed;
  useLayoutEffect(() => {
    const element = popup.current;
    const target = anchor.current;
    if (!visible || !element || !target) return;
    const position = () => {
      const bounds = target.getBoundingClientRect();
      element.style.left = `${Math.min(bounds.right + 8, window.innerWidth - element.offsetWidth - 8)}px`;
      element.style.top = `${Math.max(8, Math.min(bounds.top, window.innerHeight - element.offsetHeight - 8))}px`;
    };
    position();
    const observer = new ResizeObserver(position);
    observer.observe(element);
    observer.observe(target);
    window.addEventListener("scroll", position, true);
    window.addEventListener("resize", position);
    return () => {
      observer.disconnect();
      window.removeEventListener("scroll", position, true);
      window.removeEventListener("resize", position);
    };
  }, [visible]);
  return (
    <div
      ref={anchor}
      className={className}
      onPointerEnter={() => {
        setHovered(true);
        setDismissed(false);
      }}
      onPointerLeave={() => setHovered(false)}
      onFocusCapture={() => {
        setFocused(true);
        setDismissed(false);
      }}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget))
          setFocused(false);
      }}
      onKeyDown={(event) => {
        if (event.key === "Escape") setDismissed(true);
      }}
    >
      {children}
      <div id={id} className="sr-only">
        {details}
      </div>
      {visible &&
        createPortal(
          <div
            ref={popup}
            className="sidebar-details-tooltip"
            aria-hidden="true"
          >
            {details}
          </div>,
          document.body,
        )}
    </div>
  );
}
