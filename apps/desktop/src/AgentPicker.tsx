import { Bot, Hammer, ListChecks } from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import type { ProviderChoices } from "./workspaceState.js";

export function AgentPicker({
  value,
  current,
  choices,
  isNew,
  working,
  disabled,
  onChange,
}: {
  value: string;
  current: string | null;
  choices: ProviderChoices["agents"];
  isNew: boolean;
  working: boolean;
  disabled: boolean;
  onChange: (agent: string) => void;
}) {
  const name = (id: string) =>
    choices.find((choice) => choice.id === id)?.name ?? id;
  const pending = !isNew && Boolean(value) && value !== current;
  const quick = ["build", "plan"].filter((id) =>
    choices.some((choice) => choice.id === id),
  );
  const known = choices.some((choice) => choice.id === value);
  return (
    <div className="composer-agents">
      <div
        className="agent-switch"
        role="group"
        aria-label="Agent for next message"
      >
        {quick.map((id) => {
          const Icon = id === "build" ? Hammer : ListChecks;
          return (
            <button
              key={id}
              type="button"
              className="agent-mode"
              aria-pressed={value === id}
              title={`Use ${name(id)} for the next message`}
              disabled={disabled}
              onClick={() => onChange(id)}
            >
              <Icon size={13} aria-hidden="true" />
              {name(id)}
            </button>
          );
        })}
      </div>
      <ChoicePicker
        label="Agent"
        icon={<Bot size={13} />}
        value={value}
        triggerName={quick.includes(value) ? "More" : undefined}
        disabled={disabled || !choices.length}
        onChange={onChange}
        choices={[
          ...(isNew || !value
            ? [
                {
                  id: "",
                  name: isNew ? "Provider default" : "Agent unavailable",
                },
              ]
            : []),
          ...(value && !known
            ? [
                {
                  id: value,
                  name: value,
                  description: "Current selection · currently unavailable",
                },
              ]
            : []),
          ...choices,
        ]}
      />
      <span
        className="agent-selection"
        role="status"
        aria-label="Agent selection"
      >
        {isNew ? (
          value ? (
            `Next: ${name(value)}`
          ) : (
            "Uses provider settings"
          )
        ) : (
          <>
            {working ? "Running" : "Current"}:{" "}
            {current ? name(current) : "Not reported"}
            {pending && (
              <span className="agent-pending"> · Next: {name(value)}</span>
            )}
          </>
        )}
      </span>
    </div>
  );
}
