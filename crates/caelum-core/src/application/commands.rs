use super::Provider;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    New,
    Models,
    Variants,
    Effort,
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
    (
        "models",
        "Choose a model for this session",
        LocalCommand::Models,
    ),
    (
        "variants",
        "Choose a model effort variant",
        LocalCommand::Variants,
    ),
    (
        "effort",
        "Set the model effort for the next message",
        LocalCommand::Effort,
    ),
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
        if !matches!(*command, Self::SideQuestion | Self::Effort) && !args.trim().is_empty() {
            return Err(format!("/{name} does not take arguments."));
        }
        Ok(Some((*command, args.trim())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn models_command_opens_the_picker_for_both_providers() {
        for provider in [Provider::Codex, Provider::OpenCode] {
            assert_eq!(
                LocalCommand::parse("/models", provider),
                Ok(Some((LocalCommand::Models, "")))
            );
            assert!(LocalCommand::parse("/models fast", provider).is_err());
        }
    }

    #[test]
    fn variants_and_effort_commands_are_available_for_both_providers() {
        for provider in [Provider::Codex, Provider::OpenCode] {
            assert_eq!(
                LocalCommand::parse("/variants", provider),
                Ok(Some((LocalCommand::Variants, "")))
            );
            assert_eq!(
                LocalCommand::parse("/effort", provider),
                Ok(Some((LocalCommand::Effort, "")))
            );
            assert_eq!(
                LocalCommand::parse("/effort xhigh", provider),
                Ok(Some((LocalCommand::Effort, "xhigh")))
            );
            assert!(LocalCommand::parse("/variants xhigh", provider).is_err());
        }
    }
}
