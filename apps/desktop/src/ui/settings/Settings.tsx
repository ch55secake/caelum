import { useEffect, useRef, useState } from "react";
import { Bell, GitBranch, Info, Layers, Palette, X } from "lucide-react";
import type { WorkspaceController } from "../../application/workspace/controller.js";
import type { Preferences } from "../../application/useSettings.js";
import { AppearanceSettings } from "./AppearanceSettings.js";
import { ProviderSettings } from "./ProviderSettings.js";
import { WorktreeSettings } from "./WorktreeSettings.js";
import { AboutSettings } from "./AboutSettings.js";
import { useAbout } from "../../application/useAbout.js";
import { NotificationSettings } from "./NotificationSettings.js";

const sections = [
  {
    id: "appearance",
    name: "Appearance",
    label: "Appearance settings",
    Icon: Palette,
  },
  {
    id: "providers",
    name: "Providers",
    label: "Provider defaults",
    Icon: Layers,
  },
  {
    id: "worktrees",
    name: "Worktrees",
    label: "Worktree settings",
    Icon: GitBranch,
  },
  {
    id: "notifications",
    name: "Notifications",
    label: "Notification settings",
    Icon: Bell,
  },
  { id: "about", name: "About", label: "About Caelum", Icon: Info },
] as const;

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
  const [section, setSection] =
    useState<(typeof sections)[number]["id"]>("appearance");
  const [provider, setProvider] = useState(app.provider);
  const about = useAbout();
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
          {sections.map(({ id, name, Icon }) => (
            <button
              key={id}
              className="nav-item"
              aria-current={section === id ? "page" : undefined}
              onClick={() => setSection(id)}
            >
              <Icon size={15} />
              {name}
            </button>
          ))}
        </nav>
        <section
          className="settings-content"
          aria-label={sections.find(({ id }) => id === section)!.label}
        >
          {section === "notifications" ? (
            <NotificationSettings preferences={preferences} />
          ) : section === "about" ? (
            <AboutSettings about={about} />
          ) : section === "appearance" ? (
            <AppearanceSettings preferences={preferences} />
          ) : section === "worktrees" ? (
            <WorktreeSettings app={app} preferences={preferences} />
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
