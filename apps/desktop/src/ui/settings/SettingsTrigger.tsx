import { useEffect, useState, type ReactNode } from "react";

// Dialog focus restoration can inherit :focus-visible from a text field even
// after a pointer click. Keep return focus, but distinguish the input modality.
export function SettingsTrigger({
  open,
  className,
  children,
}: {
  open: () => void;
  className: string;
  children: ReactNode;
}) {
  const [pointer, setPointer] = useState(false);
  useEffect(() => {
    const onPointer = () => setPointer(true);
    const onKey = (event: KeyboardEvent) => {
      if (!event.metaKey && !event.ctrlKey && !event.altKey) setPointer(false);
    };
    document.addEventListener("pointerdown", onPointer, true);
    document.addEventListener("keydown", onKey, true);
    return () => {
      document.removeEventListener("pointerdown", onPointer, true);
      document.removeEventListener("keydown", onKey, true);
    };
  }, []);
  return (
    <button
      className={`${className} settings-trigger`}
      data-pointer-focus={pointer || undefined}
      aria-label="Settings"
      title="Settings (⌘,)"
      onClick={(event) => {
        event.currentTarget.focus({ preventScroll: true });
        open();
      }}
    >
      {children}
    </button>
  );
}
