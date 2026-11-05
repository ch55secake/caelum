import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ConversationHandle } from "../ui/conversation/Conversation.js";

export function useWorkspaceFocus(
  sessionKey: string,
  activeQuestionKey: string | null,
  pullRequestOpen: boolean,
) {
  const searchRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const conversationRef = useRef<ConversationHandle>(null);
  const questionDockRef = useRef<HTMLDivElement>(null);
  const [request, setRequest] = useState<{
    target: "search" | "composer";
  } | null>(null);
  useLayoutEffect(() => {
    if (!request) return;
    const target =
      request.target === "search" ? searchRef.current : textareaRef.current;
    target?.focus();
    if (request.target === "search") target?.select();
  }, [request]);
  useEffect(() => {
    if (pullRequestOpen) return;
    const previousFocus = document.activeElement;
    const frame = requestAnimationFrame(() => {
      if (document.querySelector("dialog[open]")) return;
      // Respect a newer keyboard or pointer selection made before this frame.
      if (
        document.activeElement !== previousFocus &&
        document.activeElement !== document.body
      )
        return;
      const target = activeQuestionKey
        ? questionDockRef.current?.querySelector<HTMLElement>(".question-card")
        : textareaRef.current;
      target?.focus({ preventScroll: true });
    });
    return () => cancelAnimationFrame(frame);
  }, [sessionKey, activeQuestionKey, pullRequestOpen]);
  return {
    searchRef,
    textareaRef,
    conversationRef,
    questionDockRef,
    request: (target: "search" | "composer") => setRequest({ target }),
  };
}

export type WorkspaceFocus = ReturnType<typeof useWorkspaceFocus>;
