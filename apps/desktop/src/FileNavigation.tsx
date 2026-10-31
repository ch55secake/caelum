import { createContext, useContext } from "react";
import { FileCode2, X } from "lucide-react";
import {
  fileContextLabel,
  type FileContext,
  type FileLocation,
} from "./fileContext.js";
import { copyAction, fileActions, useContextMenu } from "./ContextMenu.js";

export const FileNavigation = createContext<{
  projectPath: string;
  open: (location: FileLocation) => void;
} | null>(null);
export const useFileNavigation = () => useContext(FileNavigation);

export function FileContextChips({
  files,
  remove,
  disabled = false,
}: {
  files: FileContext[];
  remove?: (id: string) => void;
  disabled?: boolean;
}) {
  const navigation = useFileNavigation();
  const contextMenu = useContextMenu();
  if (!files.length) return null;
  return (
    <div
      className="file-context-chips"
      aria-label={remove ? "Attached context" : "Sent file context"}
    >
      {files.map((file) => (
        <span
          className="file-context-chip"
          key={file.id}
          onContextMenu={(event) => {
            if (navigation)
              contextMenu(
                event,
                fileActions(navigation.projectPath, file.path, [
                  {
                    text: "Open file",
                    action: () =>
                      navigation.open({
                        path: file.path,
                        line: file.startLine,
                      }),
                  },
                  copyAction("Copy attached snapshot", file.content),
                ]),
              );
          }}
        >
          <button
            type="button"
            title={`${fileContextLabel(file)} · Attached snapshot${file.truncated ? " · Partial snapshot (up to 64 KiB)" : ""}`}
            onClick={() =>
              navigation?.open({ path: file.path, line: file.startLine })
            }
          >
            <FileCode2 size={12} />
            <span>{fileContextLabel(file)}</span>
            {file.truncated && <small>partial</small>}
          </button>
          {remove && (
            <button
              type="button"
              aria-label={`Remove ${fileContextLabel(file)} context`}
              disabled={disabled}
              onClick={() => remove(file.id)}
            >
              <X size={11} />
            </button>
          )}
        </span>
      ))}
    </div>
  );
}
