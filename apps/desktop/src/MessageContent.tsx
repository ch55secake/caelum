import { memo, useState, type ComponentProps } from "react";
import Markdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { Check, Copy } from "lucide-react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { copyText } from "./clipboard.js";
import { useContextMenu, copyAction, fileActions } from "./ContextMenu.js";
import { useFileNavigation } from "./FileNavigation.js";
import { fileReference } from "./fileContext.js";

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
      onContextMenu={(event) =>
        contextMenu(event, [copyAction("Copy code", text)])
      }
    >
      <div className="code-heading">
        <span>{language}</span>
        <CopyButton text={text} label="Copy code" />
      </div>
      <pre>
        <code className={code?.props?.className}>{code?.props?.children}</code>
      </pre>
    </div>
  );
}
function MessageLink({ children, href }: ComponentProps<"a">) {
  const [failed, setFailed] = useState(false);
  const contextMenu = useContextMenu();
  const navigation = useFileNavigation();
  let decoded = href ?? "";
  try {
    decoded = decodeURIComponent(decoded);
  } catch {
    /* Leave malformed links intact. */
  }
  const file = navigation && fileReference(decoded, navigation.projectPath);
  if (file)
    return (
      <button
        className="file-reference"
        title={`Open ${file.path}`}
        onClick={() => navigation.open(file)}
        onContextMenu={(event) =>
          contextMenu(
            event,
            fileActions(navigation.projectPath, file.path, [
              { text: "Open file", action: () => navigation.open(file) },
            ]),
          )
        }
      >
        {children}
      </button>
    );
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

function InlineCode({ children, className }: ComponentProps<"code">) {
  const navigation = useFileNavigation();
  const contextMenu = useContextMenu();
  const text = typeof children === "string" ? children : "";
  const file =
    navigation &&
    /^[\w@./\\:#-]+$/.test(text) &&
    fileReference(text, navigation.projectPath);
  return file ? (
    <button
      className="file-reference"
      title={`Open ${file.path}`}
      onClick={() => navigation.open(file)}
      onContextMenu={(event) =>
        contextMenu(
          event,
          fileActions(navigation.projectPath, file.path, [
            { text: "Open file", action: () => navigation.open(file) },
          ]),
        )
      }
    >
      <code>{children}</code>
    </button>
  ) : (
    <code className={className}>{children}</code>
  );
}

const components: ComponentProps<typeof Markdown>["components"] = {
  pre: CodeBlock,
  a: MessageLink,
  code: InlineCode,
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
  const navigation = useFileNavigation();
  return (
    <div className="markdown">
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={components}
        urlTransform={(url) =>
          navigation && fileReference(url, navigation.projectPath)
            ? url
            : defaultUrlTransform(url)
        }
      >
        {text}
      </Markdown>
    </div>
  );
});
