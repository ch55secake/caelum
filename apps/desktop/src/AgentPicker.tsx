import { Bot, Hammer, ListChecks } from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import type { ProviderChoices } from "./workspaceState.js";

function AgentIcon({
  agent,
  current,
  working,
}: {
  agent: string;
  current: boolean;
  working: boolean;
}) {
  const Icon = agent === "build" ? Hammer : agent === "plan" ? ListChecks : Bot;
  return (
    <span
      className="agent-icon"
      data-current={current}
      data-running={current && working}
      aria-hidden="true"
    >
      <Icon size={13} />
    </span>
  );
}

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
          return (
            <button
              key={id}
              type="button"
              className="agent-mode"
              aria-pressed={value === id}
              data-current={current === id}
              title={`${current === id ? `${working ? "Running" : "Current"} agent · ` : ""}Use ${name(id)} for the next message · Shift+Tab in the message box to cycle agents`}
              disabled={disabled}
              onClick={() => onChange(id)}
            >
              <AgentIcon
                agent={id}
                current={current === id}
                working={working}
              />
              {name(id)}
            </button>
          );
        })}
      </div>
      {current && !quick.includes(current) && current !== value && (
        <span
          className="agent-custom-current"
          title={`${working ? "Running" : "Current"}: ${name(current)}`}
        >
          <AgentIcon agent={current} current working={working} />
          {name(current)}
        </span>
      )}
      <ChoicePicker
        label="Agent"
        icon={
          <AgentIcon
            agent="custom"
            current={Boolean(
              current && current === value && !quick.includes(value),
            )}
            working={working}
          />
        }
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
      <span className="sr-only" role="status" aria-label="Agent selection">
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
      {pending && (
        <span className="agent-selection agent-pending" aria-hidden="true">
          Next: {name(value)}
        </span>
      )}
    </div>
  );
}
