import { memo, useState, type ComponentProps } from "react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { Check, Copy } from "lucide-react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { copyText } from "./clipboard.js";
import { useContextMenu, copyAction } from "./ContextMenu.js";

export function CopyButton({
  text,
  label = "Copy",
}: {
  text: string;
  label?: string;
}) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  return (
    <button
      className="icon-button copy-button"
      aria-label={state === "copied" ? "Copied" : label}
      title={
        state === "failed"
          ? "Could not copy. Select the text to copy it."
          : state === "copied"
            ? "Copied"
            : label
      }
      onClick={() => {
        void copyText(text).then(
          () => setState("copied"),
          () => setState("failed"),
        );
      }}
    >
      {state === "copied" ? <Check size={13} /> : <Copy size={13} />}
    </button>
  );
}

function CodeBlock({ children }: ComponentProps<"pre">) {
  const contextMenu = useContextMenu();
  // React Markdown gives fenced blocks one code element. Read its plain text for copying.
  const code = children as {
    props?: { children?: string; className?: string };
  };
  const text = String(code?.props?.children ?? "").replace(/\n$/, "");
  const language = code?.props?.className?.replace("language-", "") ?? "text";
  return (
    <div
      className="code-block"
      onContextMenu={(event) => contextMenu(event, [copyAction("Copy code", text)])}
    >
      <div className="code-heading">
        <span>{language}</span>
        <CopyButton text={text} label="Copy code" />
      </div>
      <pre>{children}</pre>
    </div>
  );
}
function MessageLink({ children, href }: ComponentProps<"a">) {
  const [failed, setFailed] = useState(false);
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer"
      title={
        failed ? "Could not open link. Copy the address to your browser." : href
      }
      onClick={(event) => {
        if (!isTauri() || !href) return;
        event.preventDefault();
        if (/^(https?:|mailto:)/i.test(href))
          void openUrl(href).catch(() => setFailed(true));
        else setFailed(true);
      }}
    >
      {children}
    </a>
  );
}

const components: ComponentProps<typeof Markdown>["components"] = {
  pre: CodeBlock,
  a: MessageLink,
  table: ({ children }) => (
    <div className="markdown-table">
      <table>{children}</table>
    </div>
  ),
};
export const MessageContent = memo(function MessageContent({
  text,
}: {
  text: string;
}) {
  return (
    <div className="markdown">
      <Markdown remarkPlugins={[remarkGfm]} components={components}>
        {text}
      </Markdown>
    </div>
  );
});
