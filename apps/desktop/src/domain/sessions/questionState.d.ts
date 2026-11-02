import type { ProviderKind } from "./workspaceState.js";

export type QuestionValue = string | number | boolean | string[];
export type QuestionAnswer = Record<string, QuestionValue>;
export type QuestionField = {
  key: string;
  type: string;
  title?: string;
  description?: string;
  required?: boolean;
  hidden?: boolean;
  when?: { key: string; op: "eq" | "neq"; value: string | number | boolean }[];
  options?: { value: string; label: string; description?: string }[];
  custom?: boolean;
  secret?: boolean;
  default?: QuestionValue;
  placeholder?: string;
  minLength?: number;
  maxLength?: number;
  pattern?: string;
  format?: string;
  minimum?: number;
  maximum?: number;
  minItems?: number;
  maxItems?: number;
  url?: string;
};
export type QuestionPrompt = {
  provider: ProviderKind;
  projectPath: string;
  sessionId: string;
  requestId: string;
  title: string;
  fields: QuestionField[];
};
export type QuestionDraft = {
  values: QuestionAnswer;
  custom: Record<string, string>;
  other: Record<string, boolean>;
};
export function questionKey(request: QuestionPrompt): string;
export function questionDraft(request: QuestionPrompt): QuestionDraft;
export function questionAnswer(
  request: QuestionPrompt,
  draft: QuestionDraft,
): {
  answer: QuestionAnswer;
  active: string[];
  errors: Record<string, string>;
};
