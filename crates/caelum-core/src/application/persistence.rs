use super::{Attachment, Options, Provider, QueuedPrompt, Scope};
use caelum_protocol::ProjectInfo;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

mod migration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Preferences {
    pub files: BTreeMap<String, FileWorkspace>,
    pub theme: String,
    pub theme_family: String,
    pub accent_color: String,
    pub default_provider: Provider,
    pub worktree_directory: String,
    pub worktree_expanded: BTreeMap<String, bool>,
    pub providers: BTreeMap<String, Options>,
    pub favorite_models: BTreeMap<String, Vec<String>>,
    pub notifications: bool,
    pub notification_mode: String,
    pub sound_volume: u8,
    pub background_shells: bool,
    pub input_needed: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            files: BTreeMap::new(),
            theme: "system".into(),
            theme_family: "caelum".into(),
            accent_color: "violet".into(),
            default_provider: Provider::Codex,
            worktree_directory: String::new(),
            worktree_expanded: BTreeMap::new(),
            providers: BTreeMap::new(),
            favorite_models: BTreeMap::new(),
            notifications: true,
            notification_mode: "background".into(),
            sound_volume: 50,
            background_shells: true,
            input_needed: true,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileWorkspace {
    pub scroll: BTreeMap<String, [f32; 2]>,
    pub tabs: Vec<String>,
    pub selected: Option<String>,
    pub expanded: std::collections::BTreeSet<String>,
    pub line: BTreeMap<String, usize>,
}

impl Preferences {
    pub fn should_notify(&self, visible: bool) -> bool {
        self.notifications
            && self.notification_mode != "off"
            && (self.notification_mode == "always" || !visible)
    }
    /// Import the explicitly exported v1 localStorage value, never a browser DB.
    pub fn import_tauri(text: &str) -> Result<Self, String> {
        let mut value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if let Some(export) = value.get("caelum.settings.v1") {
            value = if let Some(text) = export.as_str() {
                serde_json::from_str(text).map_err(|e| e.to_string())?
            } else {
                export.clone()
            };
        }
        if !value.is_object() {
            return Err("Expected caelum.settings.v1 JSON".into());
        }
        if value.get("version").is_some_and(|v| v != 1) {
            return Err("Unsupported settings version".into());
        }
        let mut settings = Self::default();
        if let Some(theme @ ("light" | "dark" | "system")) = value["theme"].as_str() {
            settings.theme = theme.into();
        }
        if let Some(family @ ("caelum" | "vercel" | "catppuccin" | "nord" | "tokyo-night")) =
            value["themeFamily"].as_str()
        {
            settings.theme_family = family.into();
        }
        if let Some(accent @ ("theme" | "violet" | "blue" | "teal" | "green" | "amber" | "rose")) =
            value["accentColor"].as_str()
        {
            settings.accent_color = accent.into();
        }
        if value["defaultProvider"] == "opencode" {
            settings.default_provider = Provider::OpenCode;
        }
        settings.worktree_directory = value["worktreeDirectory"]
            .as_str()
            .unwrap_or_default()
            .into();
        settings.notifications = value["notifications"]["mode"] != "off";
        if let Some(mode @ ("off" | "background" | "always")) =
            value["notifications"]["mode"].as_str()
        {
            settings.notification_mode = mode.into();
        }
        settings.sound_volume = value["notifications"]["volume"]
            .as_u64()
            .unwrap_or(50)
            .min(100) as u8;
        settings.background_shells = value["notifications"]["backgroundShells"]
            .as_bool()
            .unwrap_or(true);
        settings.input_needed = value["notifications"]["inputNeeded"]
            .as_bool()
            .unwrap_or(true);
        for provider in Provider::ALL {
            let key = provider.as_str();
            let options = &value["providers"][key];
            let option = |name: &str| {
                options[name]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            settings.providers.insert(
                key.into(),
                Options {
                    model: option("model"),
                    effort: option("effort"),
                    agent: option("agent"),
                },
            );
            let mut models: Vec<String> = value["favoriteModels"][key]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect();
            models.sort();
            models.dedup();
            settings.favorite_models.insert(key.into(), models);
        }
        Ok(settings)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSession {
    pub scope: Scope,
    pub remote_id: Option<String>,
    pub title: String,
    pub options: Options,
    pub draft: String,
    pub attachments: Vec<Attachment>,
    pub failed_prompt: Option<String>,
    #[serde(default)]
    pub pending_submission: Option<QueuedPrompt>,
    pub queue: Vec<QueuedPrompt>,
    #[serde(default)]
    pub inbox: Vec<super::InboxPrompt>,
    pub archived: bool,
    pub reading: f32,
    #[serde(default)]
    pub unread: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub version: u32,
    pub projects: Vec<ProjectInfo>,
    pub sessions: Vec<SavedSession>,
    pub selected: Option<usize>,
    pub preferences: Preferences,
    #[serde(default)]
    pub remembered: BTreeMap<String, usize>,
    #[serde(default)]
    pub remembered_checkouts: BTreeMap<String, usize>,
    #[serde(default)]
    pub project_roots: BTreeMap<String, String>,
}

/// One serial writer is driven by Workspace; replacement is atomic, private, and
/// fsynced. A damaged/future file is never silently overwritten with defaults.
#[derive(Debug, Clone)]
pub struct Store(pub PathBuf);

/// Explicitly unlock before closing: a concurrently forked child can briefly
/// retain the same file description until exec closes its inherited descriptor.
pub struct StoreLock(fs::File);

impl Drop for StoreLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

impl Store {
    pub fn default_path() -> PathBuf {
        std::env::var_os("CAELUM_STATE")
            .or_else(|| std::env::var_os("CAELUM_RUNE_STATE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                    .join(".caelum/workspace-v1.json")
            })
    }
    /// Held by the desktop for its lifetime, including while its window is hidden.
    pub fn lock(&self) -> Result<StoreLock, String> {
        let parent = self
            .0
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options
            .open(self.0.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        file.try_lock()
            .map_err(|e| format!("This workspace is already open, or cannot be locked: {e}"))?;
        Ok(StoreLock(file))
    }
    pub fn load(&self) -> Result<Option<Snapshot>, String> {
        let bytes = match fs::read(&self.0) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        let mut value: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Cannot read saved workspace: {e}"))?;
        // The former black-surface override is now a complete theme. Migrate at
        // the read boundary so the next snapshot contains only the new setting.
        if value["preferences"]["blackBackgrounds"] == true {
            value["preferences"]["themeFamily"] = "vercel".into();
            value["preferences"]["accentColor"] = "theme".into();
        }
        let state: Snapshot = serde_json::from_value(value)
            .map_err(|e| format!("Cannot read saved workspace: {e}"))?;
        if state.version != 1 {
            return Err("Unsupported workspace version; saved data was preserved".into());
        }
        Ok(Some(state))
    }
    pub fn save(&self, state: &Snapshot) -> Result<(), String> {
        self.write(state)
            .map_err(|e| format!("Could not save drafts: {e}"))
    }
    fn write(&self, state: &Snapshot) -> Result<(), Box<dyn std::error::Error>> {
        let parent = self
            .0
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent)?;
        let temporary = self.0.with_extension(format!("{}.tmp", std::process::id()));
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&serde_json::to_vec(state)?)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.0)?;
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
}
