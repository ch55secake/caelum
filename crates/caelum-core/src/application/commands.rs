use super::Provider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    New,
    Timeline,
    Files,
    Changes,
    Settings,
    Usage,
    Stats,
    SideQuestion,
}

pub const COMMANDS: &[(&str, &str, LocalCommand)] = &[
    ("new", "Start a new session", LocalCommand::New),
    ("timeline", "Find an earlier prompt", LocalCommand::Timeline),
    ("files", "Browse project files", LocalCommand::Files),
    ("changes", "Review local changes", LocalCommand::Changes),
    ("settings", "Open Settings", LocalCommand::Settings),
    ("usage", "View subscription allowances", LocalCommand::Usage),
    (
        "stats",
        "View tokens, cost and activity",
        LocalCommand::Stats,
    ),
    (
        "btw",
        "Ask an independent side question",
        LocalCommand::SideQuestion,
    ),
];

impl LocalCommand {
    pub fn available(self, provider: Provider) -> bool {
        provider == Provider::OpenCode
            || !matches!(self, Self::Usage | Self::Stats | Self::SideQuestion)
    }
    pub fn parse(input: &str, provider: Provider) -> Result<Option<(Self, &str)>, String> {
        let Some(input) = input.trim().strip_prefix('/') else {
            return Ok(None);
        };
        let (name, args) = input.split_once(char::is_whitespace).unwrap_or((input, ""));
        let name = if name == "ask" { "btw" } else { name };
        let Some((_, _, command)) = COMMANDS.iter().find(|(n, _, _)| *n == name) else {
            return Ok(None);
        };
        if !command.available(provider) {
            return Err(format!("/{name} is only available with OpenCode."));
        }
        if *command != Self::SideQuestion && !args.trim().is_empty() {
            return Err(format!("/{name} does not take arguments."));
        }
        Ok(Some((*command, args.trim())))
    }
}
