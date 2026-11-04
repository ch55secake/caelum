import { useEffect } from "react";
import { Bot, Gauge, Layers, RefreshCw } from "lucide-react";
import { ChoicePicker } from "../primitives/ChoicePicker.js";
import { ModelPicker } from "../composer/ModelPicker.js";
import type { WorkspaceController } from "../../application/workspace/controller.js";
import type { Preferences } from "../../application/useSettings.js";
import type { ProviderKind } from "../../domain/sessions/workspaceState.js";
import type { ProviderDefaults } from "../../domain/settings/settingsState.js";

const providerName = (provider: ProviderKind) =>
  provider === "codex" ? "Codex" : "OpenCode";
const effortName = (name: string) =>
  name === "xhigh"
    ? "Extra high"
    : name.charAt(0).toUpperCase() + name.slice(1);

export function ProviderSettings({
  app,
  preferences,
  provider,
  setProvider,
}: {
  app: WorkspaceController;
  preferences: Preferences;
  provider: ProviderKind;
  setProvider: (provider: ProviderKind) => void;
}) {
  const { settings, update } = preferences;
  const defaults = settings.providers[provider];
  const workspace = app.workspace?.providers[provider];
  const choices = workspace?.choices;
  const loading = workspace?.isLoadingChoices;
  const error = workspace?.choicesError;
  const model = choices?.models.find(
    (choice) => choice.id === (defaults.model || choices.defaultModel),
  );
  const name = providerName(provider);
  useEffect(() => {
    if (app.project) void app.loadChoices(app.project, provider);
  }, [app.project?.path, provider]);
  function setDefault(key: keyof ProviderDefaults, value: string) {
    update((current) => ({
      ...current,
      providers: {
        ...current.providers,
        [provider]: {
          ...current.providers[provider],
          [key]: value,
          ...(key === "model" && value !== current.providers[provider].model
            ? { effort: "" }
            : {}),
        },
      },
    }));
  }
  return (
    <>
      <h2>Providers</h2>
      <p className="settings-description">
        Defaults for new sessions across your projects. Customize an individual
        draft in the composer. Star models to keep favourites at the top of both
        model pickers.
      </p>
      <div className="setting-row">
        <div>
          <strong>Default provider</strong>
          <p>Used when you open a new project.</p>
        </div>
        <ChoicePicker
          label="Default provider"
          value={settings.defaultProvider}
          icon={<Layers size={14} />}
          choices={[
            { id: "codex", name: "Codex" },
            { id: "opencode", name: "OpenCode" },
          ]}
          onChange={(value) =>
            update((current) => ({
              ...current,
              defaultProvider: value as ProviderKind,
            }))
          }
        />
      </div>
      <div
        className="provider-settings-tabs"
        role="group"
        aria-label="Provider settings"
      >
        {(["codex", "opencode"] as const).map((kind) => (
          <button
            key={kind}
            aria-pressed={kind === provider}
            onClick={() => setProvider(kind)}
          >
            {providerName(kind)}
          </button>
        ))}
      </div>
      <div className="provider-connection">
        <div>
          <span
            className="provider-connection-status"
            role="status"
            aria-label={`${name} connection status`}
            data-state={error ? "error" : choices ? "ready" : "unknown"}
          >
            <span className="connection-dot" />
            {!app.project
              ? "No project selected"
              : loading
                ? "Checking…"
                : error
                  ? "Unavailable"
                  : choices
                    ? "Available"
                    : "Not checked"}
          </span>
          <p>
            {app.project
              ? `Local ${name} · ${choices?.models.length ?? 0} models · ${app.project.name}`
              : "Open a project to check available models and agents."}
          </p>
        </div>
        <button
          className="settings-refresh"
          aria-label={`Refresh ${name} status`}
          disabled={!app.project || loading}
          onClick={() => {
            if (app.project) void app.loadChoices(app.project, provider);
          }}
        >
          <RefreshCw size={13} className={loading ? "spinning" : ""} /> Refresh
        </button>
      </div>
      {error && (
        <p className="settings-provider-error" role="alert">
          {error}
        </p>
      )}
      <div className="setting-row">
        <div>
          <strong>Model</strong>
          <p>Start new {name} sessions with this model.</p>
        </div>
        <ModelPicker
          label="Default model"
          provider={provider}
          value={defaults.model}
          disabled={!app.project || loading}
          onChange={(value) => setDefault("model", value)}
          models={choices?.models ?? []}
          favouriteModels={settings.favoriteModels[provider]}
          toggleFavourite={(id) =>
            preferences.toggleModelFavourite(provider, id)
          }
          unavailableDescription="Saved selection · currently unavailable"
          defaultChoice={{
            name: "Provider default",
            description:
              choices?.defaultModel ?? "Use the provider’s configured model",
          }}
        />
      </div>
      <div className="setting-row">
        <div>
          <strong>Reasoning effort</strong>
          <p>
            {model?.efforts.length
              ? "Available levels come from the selected model."
              : "This model uses its own reasoning settings."}
          </p>
        </div>
        <ChoicePicker
          label="Default effort"
          value={defaults.effort}
          icon={<Gauge size={14} />}
          disabled={
            !app.project ||
            loading ||
            (!model?.efforts.length && !defaults.effort)
          }
          onChange={(value) => setDefault("effort", value)}
          choices={[
            {
              id: "",
              name: "Model default",
              description: model?.defaultEffort
                ? `Use ${effortName(model.defaultEffort).toLowerCase()} effort`
                : undefined,
            },
            ...(defaults.effort &&
            !model?.efforts.some((choice) => choice.id === defaults.effort)
              ? [
                  {
                    id: defaults.effort,
                    name: effortName(defaults.effort),
                    description: "Saved selection · currently unavailable",
                  },
                ]
              : []),
            ...(model?.efforts ?? []).map((choice) => ({
              ...choice,
              name: effortName(choice.name),
            })),
          ]}
        />
      </div>
      {provider === "opencode" && (
        <div className="setting-row">
          <div>
            <strong>Agent</strong>
            <p>Choose the agent for new OpenCode sessions.</p>
          </div>
          <ChoicePicker
            label="Default agent"
            value={defaults.agent}
            icon={<Bot size={14} />}
            disabled={!app.project || loading}
            onChange={(value) => setDefault("agent", value)}
            choices={[
              { id: "", name: "Provider default" },
              ...(defaults.agent &&
              !choices?.agents.some((choice) => choice.id === defaults.agent)
                ? [
                    {
                      id: defaults.agent,
                      name: defaults.agent,
                      description: "Saved selection · currently unavailable",
                    },
                  ]
                : []),
              ...(choices?.agents ?? []),
            ]}
          />
        </div>
      )}
      <button
        className="settings-reset"
        onClick={() =>
          update((current) => ({
            ...current,
            providers: {
              ...current.providers,
              [provider]: { model: "", effort: "", agent: "" },
            },
          }))
        }
      >
        Reset {name} defaults
      </button>
    </>
  );
}
