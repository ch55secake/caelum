import { Cpu } from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import type { ModelChoice, ProviderKind } from "./workspaceState.js";

export function ModelPicker({
  label,
  provider,
  value,
  models,
  defaultChoice,
  unavailableDescription,
  favouriteModels,
  toggleFavourite,
  onChange,
  disabled,
}: {
  label: string;
  provider: ProviderKind;
  value: string;
  models: ModelChoice[];
  defaultChoice: { name: string; description: string };
  unavailableDescription: string;
  favouriteModels: string[];
  toggleFavourite: (id: string) => void;
  onChange: (id: string) => void;
  disabled?: boolean;
}) {
  const source = (id: string, model?: ModelChoice) =>
    model?.provider?.name ||
    model?.provider?.id ||
    (provider === "opencode" && id.includes("/")
      ? id.split("/")[0]
      : undefined);

  return (
    <ChoicePicker
      label={label}
      icon={<Cpu size={13} />}
      value={value}
      disabled={disabled}
      searchable
      onChange={onChange}
      favouriteIds={favouriteModels}
      onToggleFavourite={toggleFavourite}
      choices={[
        { id: "", ...defaultChoice },
        ...(value && !models.some((model) => model.id === value)
          ? [
              {
                id: value,
                name: value,
                source: source(value),
                description: unavailableDescription,
              },
            ]
          : []),
        ...models.map((model) => ({
          id: model.id,
          name: model.name,
          source: source(model.id, model),
          description: model.id,
        })),
      ]}
    />
  );
}
