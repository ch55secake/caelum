import { useCallback, useRef, useState } from "react";

type View = { search: string; searchOpen: boolean };
const initial: View = { search: "", searchOpen: false };

export function useSidebarView(path: string) {
  const [views, setViews] = useState<Record<string, View>>({});
  const positions = useRef(new Map<string, number>());
  const update = useCallback(
    (changes: Partial<View>) => {
      setViews((current) => ({
        ...current,
        [path]: { ...(current[path] ?? initial), ...changes },
      }));
    },
    [path],
  );
  return { ...(views[path] ?? initial), update, positions };
}

export type SidebarView = ReturnType<typeof useSidebarView>;
