import { useId, useRef, useState } from "react";
import { MessageCircleQuestion } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import {
  questionAnswer,
  type QuestionAnswer,
  type QuestionDraft,
  type QuestionPrompt,
  type QuestionValue,
} from "./questionState.js";

export function QuestionCard({
  request,
  draft,
  setDraft,
  resolve,
  submitting,
  error,
}: {
  request: QuestionPrompt;
  draft: QuestionDraft;
  setDraft: (draft: QuestionDraft) => void;
  resolve: (answer: QuestionAnswer | null) => void;
  submitting: boolean;
  error?: string;
}) {
  const id = useId();
  const form = useRef<HTMLFormElement>(null);
  const [attempted, setAttempted] = useState(false);
  const [linkError, setLinkError] = useState<string>();
  const result = questionAnswer(request, draft);
  function change(key: string, value: QuestionValue) {
    setDraft({ ...draft, values: { ...draft.values, [key]: value } });
  }
  function custom(key: string, value: string) {
    setDraft({ ...draft, custom: { ...draft.custom, [key]: value } });
  }
  return (
    <form
      ref={form}
      className="question-card"
      aria-label={request.title}
      aria-keyshortcuts="Control+Enter Meta+Enter"
      onKeyDown={(event) => {
        if (
          event.key === "Enter" &&
          (event.metaKey || event.ctrlKey) &&
          !event.nativeEvent.isComposing
        ) {
          event.preventDefault();
          form.current?.requestSubmit();
        }
      }}
      onSubmit={(event) => {
        event.preventDefault();
        if (submitting) return;
        setAttempted(true);
        if (Object.keys(result.errors).length) {
          requestAnimationFrame(() =>
            form.current
              ?.querySelector<HTMLElement>("[aria-invalid=true]")
              ?.focus(),
          );
          return;
        }
        resolve(result.answer);
      }}
    >
      <div className="question-title">
        <MessageCircleQuestion size={16} />
        <strong>{request.title}</strong>
      </div>
      <p className="question-status">Waiting for your answer</p>
      <fieldset className="question-fields" disabled={submitting}>
        {request.fields
          .filter((field) => result.active.includes(field.key))
          .map((field, index) => {
            const fieldId = `${id}-${index}`;
            const label = field.title || field.key;
            const value = Object.hasOwn(draft.values, field.key)
              ? draft.values[field.key]
              : undefined;
            const other = draft.other[field.key] === true;
            const customValue = Object.hasOwn(draft.custom, field.key)
              ? draft.custom[field.key]
              : "";
            const invalid =
              attempted && Object.hasOwn(result.errors, field.key)
                ? result.errors[field.key]
                : undefined;
            const inputProps = {
              "aria-labelledby": `${fieldId}-title`,
              "aria-describedby": `${fieldId}-description${invalid ? ` ${fieldId}-error` : ""}`,
              "aria-invalid": Boolean(invalid),
            };
            if (field.hidden)
              return invalid ? (
                <p key={field.key} role="alert">
                  {label}: {invalid}
                </p>
              ) : null;
            if (field.type === "external")
              return (
                <div key={field.key} className="question-field">
                  <p>{field.description}</p>
                  <button
                    type="button"
                    onClick={() => {
                      if (!field.url || !/^https?:\/\//i.test(field.url)) {
                        setLinkError("This question has an unsupported link.");
                        return;
                      }
                      void openUrl(field.url).catch((reason: unknown) =>
                        setLinkError(String(reason)),
                      );
                    }}
                  >
                    {label} ↗
                  </button>
                </div>
              );
            const multiple = field.type === "multiselect";
            const choices =
              field.type === "boolean"
                ? [
                    { value: true, label: "Yes", description: "" },
                    { value: false, label: "No", description: "" },
                  ]
                : (field.options ?? []);
            return (
              <fieldset key={field.key} className="question-field">
                <legend id={`${fieldId}-title`}>
                  {label}
                  {!field.required && <span> (optional)</span>}
                </legend>
                <p id={`${fieldId}-description`}>
                  {field.description}
                  {multiple && (
                    <span className="question-hint">
                      Select all that apply.
                    </span>
                  )}
                </p>
                {choices.map((option, optionIndex) => (
                  <label className="question-option" key={String(option.value)}>
                    <input
                      aria-describedby={inputProps["aria-describedby"]}
                      aria-invalid={Boolean(invalid)}
                      type={multiple ? "checkbox" : "radio"}
                      name={fieldId}
                      value={String(optionIndex)}
                      checked={
                        multiple
                          ? Array.isArray(value) &&
                            value.includes(String(option.value))
                          : !other && value === option.value
                      }
                      onChange={(event) => {
                        if (multiple)
                          change(
                            field.key,
                            event.target.checked
                              ? [
                                  ...(Array.isArray(value) ? value : []),
                                  String(option.value),
                                ]
                              : (Array.isArray(value) ? value : []).filter(
                                  (item) => item !== option.value,
                                ),
                          );
                        else
                          setDraft({
                            ...draft,
                            values: {
                              ...draft.values,
                              [field.key]: option.value,
                            },
                            other: { ...draft.other, [field.key]: false },
                          });
                      }}
                    />
                    <span>
                      <strong>{option.label}</strong>
                      {option.description && (
                        <small>{option.description}</small>
                      )}
                    </span>
                  </label>
                ))}
                {field.custom && choices.length > 0 && !multiple && (
                  <label className="question-option">
                    <input
                      type="radio"
                      name={fieldId}
                      checked={other}
                      onChange={() =>
                        setDraft({
                          ...draft,
                          other: { ...draft.other, [field.key]: true },
                        })
                      }
                    />
                    <span>Type your own answer</span>
                  </label>
                )}
                {multiple && field.custom ? (
                  <textarea
                    aria-label={`${label}: custom answers`}
                    aria-describedby={inputProps["aria-describedby"]}
                    aria-invalid={Boolean(invalid)}
                    placeholder="Other answers, one per line"
                    rows={2}
                    value={customValue}
                    onChange={(event) => custom(field.key, event.target.value)}
                  />
                ) : field.type === "string" &&
                  !field.secret &&
                  !field.format &&
                  (!choices.length || other) ? (
                  <textarea
                    {...inputProps}
                    rows={2}
                    placeholder={field.placeholder}
                    maxLength={field.maxLength}
                    value={String((other ? customValue : value) ?? "")}
                    onChange={(event) =>
                      other
                        ? custom(field.key, event.target.value)
                        : change(field.key, event.target.value)
                    }
                  />
                ) : (field.type === "string" ||
                    field.type === "number" ||
                    field.type === "integer") &&
                  (!choices.length || other) ? (
                  <input
                    {...inputProps}
                    className="question-input"
                    autoComplete="off"
                    type={
                      field.secret
                        ? "password"
                        : field.type === "number" || field.type === "integer"
                          ? "number"
                          : field.format === "email"
                            ? "email"
                            : field.format === "uri"
                              ? "url"
                              : field.format === "date"
                                ? "date"
                                : "text"
                    }
                    step={field.type === "integer" ? 1 : "any"}
                    min={field.minimum}
                    max={field.maximum}
                    placeholder={field.placeholder}
                    maxLength={field.maxLength}
                    value={String((other ? customValue : value) ?? "")}
                    onChange={(event) =>
                      other
                        ? custom(field.key, event.target.value)
                        : change(field.key, event.target.value)
                    }
                  />
                ) : null}
                {invalid && (
                  <p
                    id={`${fieldId}-error`}
                    className="question-error"
                    role="alert"
                  >
                    {invalid}
                  </p>
                )}
              </fieldset>
            );
          })}
      </fieldset>
      {(error || linkError) && (
        <p className="question-error" role="alert">
          {error || linkError}
        </p>
      )}
      <div className="question-actions">
        <button
          type="button"
          disabled={submitting}
          onClick={() => resolve(null)}
        >
          Cancel questions
        </button>
        <button type="submit" className="question-submit" disabled={submitting}>
          {submitting ? "Sending…" : error ? "Retry answer" : "Submit answers"}
        </button>
      </div>
    </form>
  );
}
