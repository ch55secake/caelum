import { useEffect, useRef, useState, type CSSProperties } from "react";
import {
  Bot,
  Check,
  Gauge,
  GitBranch,
  FolderOpen,
  Layers,
  Monitor,
  Moon,
  Palette,
  RefreshCw,
  Sun,
  X,
} from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import { ModelPicker } from "./ModelPicker.js";
import { accentColors, type AccentColor } from "./accentColors.js";
import type { WorkspaceController } from "./Sidebar.js";
import type { useSettings } from "./useSettings.js";
import type { ProviderKind } from "./workspaceState.js";
import type { ProviderDefaults } from "./settingsState.js";
import { open } from "@tauri-apps/plugin-dialog";

type Preferences = ReturnType<typeof useSettings>;
const providerName = (provider: ProviderKind) =>
  provider === "codex" ? "Codex" : "OpenCode";
const effortName = (name: string) =>
  name === "xhigh"
    ? "Extra high"
    : name.charAt(0).toUpperCase() + name.slice(1);

function ProviderSettings({
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

export function Settings({
  app,
  preferences,
  close,
}: {
  app: WorkspaceController;
  preferences: Preferences;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [section, setSection] = useState<
    "appearance" | "providers" | "worktrees"
  >("appearance");
  const [provider, setProvider] = useState(app.provider);
  useEffect(() => {
    const element = dialog.current!;
    const previous = document.activeElement;
    element.showModal();
    return () => {
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus({ preventScroll: true });
    };
  }, []);
  return (
    <dialog
      ref={dialog}
      className="settings-dialog"
      aria-labelledby="settings-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
      onClick={(event) => {
        if (event.target !== event.currentTarget) return;
        const rect = event.currentTarget.getBoundingClientRect();
        if (
          event.clientX < rect.left ||
          event.clientX > rect.right ||
          event.clientY < rect.top ||
          event.clientY > rect.bottom
        )
          close();
      }}
    >
      <header className="settings-heading">
        <h1 id="settings-title">Settings</h1>
        <button
          className="icon-button"
          aria-label="Close settings"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="settings-layout">
        <nav className="settings-nav" aria-label="Settings sections">
          <button
            className="nav-item"
            aria-current={section === "appearance" ? "page" : undefined}
            onClick={() => setSection("appearance")}
          >
            <Palette size={15} /> Appearance
          </button>
          <button
            className="nav-item"
            aria-current={section === "providers" ? "page" : undefined}
            onClick={() => setSection("providers")}
          >
            <Layers size={15} /> Providers
          </button>
          <button
            className="nav-item"
            aria-current={section === "worktrees" ? "page" : undefined}
            onClick={() => setSection("worktrees")}
          >
            <GitBranch size={15} /> Worktrees
          </button>
        </nav>
        <section
          className="settings-content"
          aria-label={
            section === "appearance"
              ? "Appearance settings"
              : section === "worktrees"
                ? "Worktree settings"
                : "Provider defaults"
          }
        >
          {section === "appearance" ? (
            <>
              <h2>Appearance</h2>
              <p className="settings-description">
                Choose how Caelum looks on this device.
              </p>
              <fieldset className="theme-options">
                <legend>Theme</legend>
                {(
                  [
                    { id: "dark", name: "Dark", Icon: Moon },
                    { id: "light", name: "Light", Icon: Sun },
                    { id: "system", name: "System", Icon: Monitor },
                  ] as const
                ).map(({ id, name, Icon }) => (
                  <label className="theme-option" key={id}>
                    <input
                      type="radio"
                      name="theme"
                      value={id}
                      checked={preferences.settings.theme === id}
                      onChange={() =>
                        preferences.update((current) => ({
                          ...current,
                          theme: id,
                        }))
                      }
                    />
                    <span
                      className={`theme-preview theme-preview-${id}`}
                      aria-hidden="true"
                    >
                      <span />
                      <span>
                        <i />
                        <i />
                        <i />
                      </span>
                    </span>
                    <span className="theme-option-name">
                      <Icon size={14} />
                      {name}
                      {preferences.settings.theme === id && <Check size={14} />}
                    </span>
                  </label>
                ))}
              </fieldset>
              <p className="settings-note">
                System follows your device’s appearance automatically.
              </p>
              <fieldset className="accent-options">
                <legend>Accent colour</legend>
                {Object.entries(accentColors).map(([id, palette]) => (
                  <label className="accent-option" key={id}>
                    <input
                      type="radio"
                      name="accent-color"
                      value={id}
                      checked={preferences.settings.accentColor === id}
                      onChange={() =>
                        preferences.update((current) => ({
                          ...current,
                          accentColor: id as AccentColor,
                        }))
                      }
                    />
                    <span
                      className="accent-swatch"
                      style={
                        {
                          "--swatch-color": palette[preferences.resolvedTheme],
                        } as CSSProperties
                      }
                      aria-hidden="true"
                    >
                      {preferences.settings.accentColor === id && (
                        <Check size={15} />
                      )}
                    </span>
                    <span>{palette.name}</span>
                  </label>
                ))}
              </fieldset>
              <p className="settings-note">
                Highlights, links, and controls adapt to your theme.
              </p>
              {preferences.themeError && (
                <p className="settings-provider-error" role="alert">
                  {preferences.themeError}
                </p>
              )}
            </>
          ) : section === "worktrees" ? (
            <>
              <h2>Worktrees</h2>
              <p className="settings-description">
                New checkouts are grouped by project in this folder.
              </p>
              <label className="worktree-folder-setting">
                Worktree folder
                <input
                  value={preferences.settings.worktreeDirectory}
                  placeholder="~/.caelum/worktrees"
                  onChange={(event) =>
                    preferences.update((current) => ({
                      ...current,
                      worktreeDirectory: event.target.value,
                    }))
                  }
                />
              </label>
              <button
                className="settings-refresh"
                onClick={() => {
                  void open({
                    directory: true,
                    multiple: false,
                    title: "Choose worktree folder",
                  })
                    .then((path) => {
                      if (typeof path === "string")
                        preferences.update((current) => ({
                          ...current,
                          worktreeDirectory: path,
                        }));
                    })
                    .catch((reason) => app.setError(String(reason)));
                }}
              >
                <FolderOpen size={14} /> Choose folder
              </button>
              <p className="settings-note">
                Leave empty to use ~/.caelum/worktrees. Changes apply to newly
                created worktrees; existing checkouts keep their locations.
              </p>
            </>
          ) : (
            <ProviderSettings
              app={app}
              preferences={preferences}
              provider={provider}
              setProvider={setProvider}
            />
          )}
        </section>
      </div>
      <footer className="settings-footer">
        {preferences.storageError ? (
          <>
            <span role="alert">{preferences.storageError}</span>
            <button onClick={preferences.retrySave}>Retry saving</button>
          </>
        ) : (
          <span>Changes are saved automatically on this device.</span>
        )}
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
