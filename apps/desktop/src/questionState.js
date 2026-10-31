export function questionKey(request) {
  return JSON.stringify([
    request.provider,
    request.projectPath,
    request.sessionId,
    request.requestId,
  ]);
}

export function questionDraft(request) {
  const values = Object.create(null),
    custom = Object.create(null),
    other = Object.create(null);
  for (const field of request.fields) {
    if (field.default === undefined) continue;
    values[field.key] = field.default;
    if (!field.custom || !field.options?.length) continue;
    const options = field.options.map((option) => option.value);
    if (field.type === "multiselect" && Array.isArray(field.default)) {
      values[field.key] = field.default.filter((value) =>
        options.includes(value),
      );
      custom[field.key] = field.default
        .filter((value) => !options.includes(value))
        .join("\n");
    } else if (
      typeof field.default === "string" &&
      !options.includes(field.default)
    ) {
      other[field.key] = true;
      custom[field.key] = field.default;
    }
  }
  return { values: { ...values }, custom: { ...custom }, other: { ...other } };
}

// Resolve conditions in field order: inactive answers must not activate later
// fields or be sent to the provider, even when a draft still contains them.
export function questionAnswer(request, draft) {
  const answer = Object.create(null),
    errors = Object.create(null),
    active = [];
  for (const field of request.fields) {
    if (
      field.when?.some((condition) => {
        const value = answer[condition.key];
        if (value === undefined) return true;
        const matches = Array.isArray(value)
          ? value.includes(condition.value)
          : value === condition.value;
        return condition.op === "eq" ? !matches : matches;
      })
    )
      continue;
    active.push(field.key);
    if (field.type === "external") continue;
    let value = Object.hasOwn(draft.values, field.key)
      ? draft.values[field.key]
      : undefined;
    const custom = Object.hasOwn(draft.custom, field.key)
      ? draft.custom[field.key]
      : "";
    if (field.type === "multiselect") {
      value = [
        ...new Set([
          ...(Array.isArray(value) ? value : []),
          ...(field.custom
            ? custom
                .split("\n")
                .map((part) => part.trim())
                .filter(Boolean)
            : []),
        ]),
      ];
    } else if (draft.other[field.key] === true && field.custom) {
      value = custom;
    }
    const missing = value === undefined || value === "";
    if (missing && !field.required) continue;
    let error;
    if (missing) error = "Please answer this question.";
    else if (field.type === "number" || field.type === "integer") {
      value = Number(value);
      if (
        !Number.isFinite(value) ||
        (field.type === "integer" && !Number.isInteger(value))
      )
        error =
          field.type === "integer"
            ? "Enter a whole number."
            : "Enter a number.";
      else if (field.minimum !== undefined && value < field.minimum)
        error = `Enter at least ${field.minimum}.`;
      else if (field.maximum !== undefined && value > field.maximum)
        error = `Enter at most ${field.maximum}.`;
    } else if (field.type === "string") {
      if (typeof value !== "string" || (field.required && !value.trim()))
        error = "Please answer this question.";
      else if (
        field.options?.length &&
        !field.custom &&
        !field.options.some((option) => option.value === value)
      )
        error = "Choose an available answer.";
      else if (field.minLength !== undefined && value.length < field.minLength)
        error = `Use at least ${field.minLength} characters.`;
      else if (field.maxLength !== undefined && value.length > field.maxLength)
        error = `Use at most ${field.maxLength} characters.`;
      else if (field.pattern) {
        try {
          if (!new RegExp(field.pattern).test(value))
            error = "Use the requested format.";
        } catch {
          error = "This question has an invalid validation pattern.";
        }
      }
    } else if (field.type === "multiselect") {
      const minimum = field.minItems ?? (field.required ? 1 : 0);
      if (value.length < minimum)
        error = `Choose at least ${minimum} answer${minimum === 1 ? "" : "s"}.`;
      else if (field.maxItems !== undefined && value.length > field.maxItems)
        error = `Choose at most ${field.maxItems} answers.`;
      else if (
        !field.custom &&
        value.some(
          (item) => !field.options?.some((option) => option.value === item),
        )
      )
        error = "Choose available answers.";
    } else if (field.type === "boolean") {
      if (typeof value !== "boolean") error = "Choose Yes or No.";
    } else error = `Unsupported question type: ${field.type}`;
    if (error) errors[field.key] = error;
    else if (!missing) answer[field.key] = value;
  }
  return { answer: { ...answer }, errors: { ...errors }, active };
}
