import { useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink as LinkIcon } from "lucide-react";

export function ExternalLink({
  href,
  children,
}: {
  href: string;
  children: ReactNode;
}) {
  const [failed, setFailed] = useState(false);
  return (
    <span className="external-link">
      <a
        href={href}
        target="_blank"
        rel="noreferrer"
        onClick={(event) => {
          if (!isTauri()) return;
          event.preventDefault();
          void openUrl(href).then(
            () => setFailed(false),
            () => setFailed(true),
          );
        }}
      >
        {children}
        <LinkIcon size={12} aria-hidden="true" />
      </a>
      {failed && (
        <span role="alert">
          Could not open the link. Try again or copy its address.
        </span>
      )}
    </span>
  );
}
