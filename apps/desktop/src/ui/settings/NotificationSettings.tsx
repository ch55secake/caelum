import { useState } from "react";
import { Volume2 } from "lucide-react";
import type { Preferences } from "../../application/useSettings.js";
import { playSound } from "../../application/sounds.js";

export function NotificationSettings({
  preferences,
}: {
  preferences: Preferences;
}) {
  const { notifications } = preferences.settings;
  const [error, setError] = useState("");
  const change = (patch: Partial<typeof notifications>) =>
    preferences.update((settings) => ({
      ...settings,
      notifications: { ...settings.notifications, ...patch },
    }));
  return (
    <>
      <h2>Notifications</h2>
      <p className="settings-description">
        A gentle sound when work finishes or needs your input.
      </p>
      <fieldset className="theme-options notification-options">
        <legend>Completion sounds</legend>
        {(
          [
            { id: "off", name: "Off" },
            { id: "background", name: "Background only" },
            { id: "always", name: "Always" },
          ] as const
        ).map(({ id, name }) => (
          <label className="theme-option" key={id}>
            <input
              type="radio"
              name="notification-mode"
              checked={notifications.mode === id}
              onChange={() => change({ mode: id })}
            />
            <span className="theme-option-name">{name}</span>
          </label>
        ))}
      </fieldset>
      <p className="settings-note">
        Background only plays when the latest response is not visible in the
        foreground.
      </p>
      <label className="sound-volume">
        Volume <output>{notifications.volume}%</output>
        <input
          aria-label="Sound volume"
          type="range"
          min="0"
          max="100"
          value={notifications.volume}
          onChange={(event) => change({ volume: Number(event.target.value) })}
        />
      </label>
      <button
        className="test-sound"
        disabled={notifications.volume === 0}
        onClick={() => {
          setError("");
          void playSound(notifications.volume).catch((reason) =>
            setError(String(reason)),
          );
        }}
      >
        <Volume2 size={14} />
        Test sound
      </button>
      <label className="sound-option">
        <input
          type="checkbox"
          checked={notifications.backgroundShells}
          onChange={(event) =>
            change({ backgroundShells: event.target.checked })
          }
        />
        Background shell completions
      </label>
      <label className="sound-option">
        <input
          type="checkbox"
          checked={notifications.inputNeeded}
          onChange={(event) => change({ inputNeeded: event.target.checked })}
        />
        Approvals and questions needing input
      </label>
      {error && (
        <p className="settings-provider-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}
