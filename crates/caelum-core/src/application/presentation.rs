//! Renderer-independent labels, outcomes and summaries for both live and restored work.
use super::{Entry, EntryKind};
use caelum_protocol::GitHubStatusCheck;
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct Media {
    pub name: String,
    pub source: String,
}

fn markdown_images() -> &'static regex::Regex {
    static REGEX: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    REGEX.get_or_init(|| regex::Regex::new(r"!\[([^\]]*)\]\(([^\s)]+)\)").expect("image regex"))
}

pub fn visible_text(text: &str) -> String {
    markdown_parts(text, false).0
}

pub fn markdown_media(text: &str) -> Vec<Media> {
    markdown_parts(text, true).1
}

/// The same presentation is used for transcript, navigation, selection and copying.
/// Provider records remain intact; incomplete markup is withheld until it is unambiguous.
pub fn entry_text(entry: &Entry) -> String {
    let text = if entry.kind == EntryKind::User {
        super::split_context(&entry.text).0
    } else {
        &entry.text
    };
    markdown_parts(text, !entry.complete).0
}

pub fn entry_visible(entry: &Entry) -> bool {
    entry.kind == EntryKind::Tool
        || (!entry.text.contains('<') && !entry.text.trim().is_empty())
        || !entry_text(entry).trim().is_empty()
        || !entry.media.is_empty()
        || (entry.kind == EntryKind::User && !super::split_context(&entry.text).1.is_empty())
}

/// Byte ranges on the original text, including case-folded Unicode matches.
pub fn search_ranges(text: &str, query: &str) -> Vec<std::ops::Range<usize>> {
    if query.trim().is_empty() {
        return Vec::new();
    }
    regex::RegexBuilder::new(&regex::escape(query))
        .case_insensitive(true)
        .build()
        .map(|re| re.find_iter(text).map(|m| m.range()).collect())
        .unwrap_or_default()
}

// Keep literal code and whitespace intact. Only presentation markup outside code
// is hidden; in particular an internal reminder must never trigger an image read.
fn markdown_parts(text: &str, streaming: bool) -> (String, Vec<Media>) {
    let mut fence: Option<(char, usize)> = None;
    let mut reminder = false;
    let mut output = String::new();
    let mut media = Vec::new();
    for line in text.split_inclusive('\n') {
        if !reminder && (line.starts_with("    ") || line.starts_with('\t')) {
            output.push_str(line);
            continue;
        }
        let trimmed = line.trim_start();
        let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'));
        let length = marker.map_or(0, |c| trimmed.chars().take_while(|v| *v == c).count());
        if !reminder && length >= 3 {
            if let Some((c, n)) = fence {
                if marker == Some(c) && length >= n && trimmed[length..].trim().is_empty() {
                    fence = None;
                }
            } else {
                fence = marker.map(|c| (c, length));
            }
            output.push_str(line);
            continue;
        }
        if fence.is_some() {
            output.push_str(line);
            continue;
        }
        let mut rest = line;
        while !rest.is_empty() {
            if reminder {
                if let Some(end) = rest.find("</system-reminder>") {
                    rest = &rest[end + 18..];
                    reminder = false;
                } else {
                    break;
                }
                continue;
            }
            if rest.starts_with("<system-reminder>") {
                reminder = true;
                rest = &rest[17..];
                continue;
            }
            if streaming && !line.ends_with('\n') && "<system-reminder>".starts_with(rest) {
                break;
            }
            if rest.starts_with('`') {
                let n = rest.bytes().take_while(|b| *b == b'`').count();
                if let Some(end) = rest[n..].find(&rest[..n]) {
                    output.push_str(&rest[..end + 2 * n]);
                    rest = &rest[end + 2 * n..];
                    continue;
                }
            }
            if let Some(m) = rest
                .starts_with("![")
                .then(|| markdown_images().captures(rest))
                .flatten()
                .filter(|m| m.get(0).is_some_and(|m| m.start() == 0))
            {
                output.push_str(&m[1]);
                media.push(Media {
                    name: m[1].into(),
                    source: m[2].trim_matches(['<', '>']).into(),
                });
                rest = &rest[m.get(0).unwrap().end()..];
                continue;
            }
            let c = rest.chars().next().unwrap();
            output.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    (output, media)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActivityKind {
    Working,
    Thinking,
    Responding,
    Tool,
    Retrying,
    Input,
    Background,
    Stopping,
}

pub struct Activity {
    pub kind: ActivityKind,
    pub label: String,
}

/// Activity belongs to a session, not to the most recently painted output chunk.
pub fn activity(session: &super::Session, needs_input: bool) -> Option<Activity> {
    if session.disconnected {
        return None;
    }
    let (kind, label) = if session.stopping {
        (ActivityKind::Stopping, "Stopping…".into())
    } else if needs_input {
        (ActivityKind::Input, "Waiting for input".into())
    } else if session.submitting {
        (ActivityKind::Working, "Working…".into())
    } else if session.conversation.working {
        if session.conversation.retrying {
            (ActivityKind::Retrying, "Retrying…".into())
        } else if let Some(entry) = session
            .conversation
            .entries
            .iter()
            .rev()
            .take_while(|e| e.kind != EntryKind::User)
            .find(|e| !e.complete && !e.background)
        {
            match entry.kind {
                EntryKind::Reasoning => (ActivityKind::Thinking, "Thinking…".into()),
                EntryKind::Assistant => (ActivityKind::Responding, "Responding…".into()),
                EntryKind::Tool => {
                    let tool = Tool::from_entry(entry, 0);
                    (
                        ActivityKind::Tool,
                        if tool.name == "Run" {
                            "Running a command…".into()
                        } else {
                            format!("{}…", tool.name)
                        },
                    )
                }
                EntryKind::Compaction => (ActivityKind::Working, "Compacting context…".into()),
                _ => (ActivityKind::Working, "Working…".into()),
            }
        } else if !session.shells.is_empty() {
            (
                ActivityKind::Background,
                "Waiting for background commands".into(),
            )
        } else {
            (ActivityKind::Working, "Working…".into())
        }
    } else if !session.shells.is_empty() {
        (
            ActivityKind::Background,
            "Waiting for background commands".into(),
        )
    } else {
        return None;
    };
    Some(Activity { kind, label })
}

pub fn media(value: &Value) -> Vec<Media> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| {
            let mime = part["mime"]
                .as_str()
                .or(part["mimeType"].as_str())
                .unwrap_or_default();
            if !mime.starts_with("image/")
                && !part["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.starts_with("data:image/"))
                && !matches!(
                    part["type"].as_str(),
                    Some("image" | "localImage" | "input_image")
                )
            {
                return None;
            }
            let source = part["uri"]
                .as_str()
                .or(part["url"].as_str())
                .or(part["path"].as_str())
                .map(str::to_owned)
                .or_else(|| {
                    part["hash"]
                        .as_str()
                        .map(|hash| format!("opencode-media:{hash}"))
                })
                .or_else(|| {
                    part["data"]
                        .as_str()
                        .map(|data| format!("data:{mime};base64,{data}"))
                })?;
            Some(Media {
                source,
                name: part["name"].as_str().unwrap_or("Image").into(),
            })
        })
        .collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Outcome {
    Running,
    Success,
    Failure,
    Interrupted,
    Neutral,
    Unknown,
}

pub fn outcome(status: &str, complete: bool) -> Outcome {
    match status.to_ascii_lowercase().as_str() {
        "success" | "succeeded" | "completed" => {
            if complete {
                Outcome::Success
            } else {
                Outcome::Running
            }
        }
        s if s.contains("fail")
            || matches!(
                s,
                "error" | "timed_out" | "action_required" | "startup_failure"
            ) =>
        {
            Outcome::Failure
        }
        "cancelled" | "canceled" | "interrupted" => Outcome::Interrupted,
        "neutral" | "skipped" => Outcome::Neutral,
        "running" | "inprogress" | "in_progress" | "queued" | "pending" | "requested"
        | "waiting" => Outcome::Running,
        _ if !complete => Outcome::Running,
        _ => Outcome::Unknown,
    }
}

pub fn duration(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds >= 3600 {
        format!("{}h {}m", seconds / 3600, seconds % 3600 / 60)
    } else if seconds >= 60 {
        format!("{}m {}s", seconds / 60, seconds % 60)
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct Tool {
    pub name: String,
    pub summary: String,
    pub paths: Vec<String>,
    pub patch: String,
    pub input: String,
    pub outcome: Outcome,
    pub details: Vec<String>,
    pub timing: String,
}

impl Tool {
    pub fn from_entry(entry: &Entry, now: u64) -> Self {
        let input: Value =
            serde_json::from_str(&entry.input).unwrap_or(Value::String(entry.input.clone()));
        let name = match entry.kind {
            EntryKind::Reasoning => {
                if entry.complete {
                    "Thought"
                } else {
                    "Thinking"
                }
            }
            EntryKind::Compaction => "Context compaction",
            _ => match entry.name.as_str() {
                "commandExecution" | "shell" | "bash" | "execute" | "Background shell" => "Run",
                "fileChange" | "apply_patch" | "edit" | "patch" | "write" => "Patch",
                "read" | "read_file" => "Read",
                "grep" | "glob" | "search" | "webSearch" | "websearch" => "Search",
                "" => "Tool",
                other => other,
            },
        }
        .to_owned();
        let mut paths = Vec::new();
        collect_paths(&input, &mut paths);
        paths.sort();
        paths.dedup();
        let summary = if matches!(entry.kind, EntryKind::Reasoning | EntryKind::Compaction) {
            entry_text(entry)
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or_default()
                .trim_matches('*')
                .into()
        } else {
            [
                "command",
                "cmd",
                "description",
                "path",
                "filePath",
                "file_path",
                "pattern",
                "query",
            ]
            .iter()
            .find_map(|key| input[*key].as_str())
            .map(str::to_owned)
            .or_else(|| input.as_str().map(str::to_owned))
            .unwrap_or_else(|| {
                if !paths.is_empty() {
                    paths.join(", ")
                } else {
                    entry.input.lines().next().unwrap_or_default().into()
                }
            })
        };
        let mut details = Vec::new();
        if let Some(cwd) = input["cwd"]
            .as_str()
            .or(entry.metadata["cwd"].as_str())
            .or(input["workdir"].as_str())
        {
            details.push(cwd.into());
        }
        if entry.background || input["background"] == true || entry.metadata["background"] == true {
            details.push("Background".into());
        }
        if let Some(exit) = entry.metadata["exitCode"]
            .as_i64()
            .or(entry.metadata["exit"].as_i64())
        {
            details.push(format!("Exit {exit}"));
        }
        if let Some(signal) = entry.metadata["signal"].as_str() {
            details.push(signal.into());
        }
        let elapsed = entry.metadata["durationMs"].as_u64().or_else(|| {
            entry
                .started_at
                .zip(
                    entry.metadata["ended"]
                        .as_u64()
                        .or_else(|| (!entry.complete).then_some(now)),
                )
                .map(|(start, end)| end.saturating_sub(start))
        });
        let timing = elapsed.map(duration).unwrap_or_default();
        let patch = if name == "Patch" {
            patch_text(&input)
        } else {
            String::new()
        };
        let mut remaining = input.clone();
        if matches!(name.as_str(), "Read" | "Run" | "Patch") {
            if name == "Read" {
                for (key, label) in [("offset", "From"), ("limit", "Limit")] {
                    if let Some(n) = input[key].as_u64() {
                        details.push(format!("{label} {n}"));
                    }
                }
            }
            trim_tool_input(&mut remaining, &name, &patch, &summary);
        }
        let input = match remaining {
            Value::Null => String::new(),
            Value::String(text) => text,
            value => serde_json::to_string_pretty(&value).unwrap_or_default(),
        };
        let state = if !entry.error.is_empty() {
            Outcome::Failure
        } else {
            outcome(&entry.status, entry.complete)
        };
        Self {
            name,
            summary: summary.chars().take(180).collect(),
            paths,
            patch,
            input,
            outcome: state,
            details,
            timing,
        }
    }
}

fn trim_tool_input(input: &mut Value, name: &str, patch: &str, summary: &str) {
    match input {
        Value::Object(fields) => {
            fields.retain(|key, value| {
                if matches!(
                    key.as_str(),
                    "path"
                        | "filePath"
                        | "file_path"
                        | "filename"
                        | "cwd"
                        | "workdir"
                        | "background"
                ) {
                    return false;
                }
                if name == "Read" && matches!(key.as_str(), "offset" | "limit") && value.is_u64() {
                    return false;
                }
                if name == "Patch"
                    && !patch.is_empty()
                    && matches!(
                        key.as_str(),
                        "patch"
                            | "patchText"
                            | "diff"
                            | "oldString"
                            | "old_string"
                            | "newString"
                            | "new_string"
                    )
                {
                    return false;
                }
                !(matches!(key.as_str(), "command" | "cmd" | "description")
                    && value.as_str() == Some(summary)
                    && summary.chars().count() <= 180)
            });
            if fields.is_empty() {
                *input = Value::Null;
            }
        }
        Value::Array(values) => {
            for value in values.iter_mut() {
                trim_tool_input(value, name, patch, summary);
            }
            values.retain(|v| !v.is_null());
            if values.is_empty() {
                *input = Value::Null;
            }
        }
        Value::String(value)
            if (name == "Patch" && !patch.is_empty())
                || (value == summary && summary.chars().count() <= 180) =>
        {
            *input = Value::Null
        }
        _ => {}
    }
}

fn collect_paths(input: &Value, out: &mut Vec<String>) {
    match input {
        Value::Object(map) => {
            for (key, value) in map {
                if matches!(key.as_str(), "path" | "filePath" | "file_path" | "filename") {
                    if let Some(path) = value.as_str() {
                        out.push(path.into());
                    }
                } else if value.is_array() || value.is_object() {
                    collect_paths(value, out);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_paths(value, out);
            }
        }
        _ => {}
    }
}

fn patch_text(input: &Value) -> String {
    if let Some(text) = input.as_str() {
        return text.into();
    }
    for key in ["patch", "patchText", "diff"] {
        if let Some(text) = input[key].as_str() {
            return text.into();
        }
    }
    if let Some(old) = input["oldString"].as_str().or(input["old_string"].as_str()) {
        let new = input["newString"]
            .as_str()
            .or(input["new_string"].as_str())
            .unwrap_or_default();
        return old
            .lines()
            .map(|l| format!("-{l}\n"))
            .chain(new.lines().map(|l| format!("+{l}\n")))
            .collect();
    }
    if let Some(values) = input.as_array() {
        return values.iter().map(patch_text).collect::<Vec<_>>().join("\n");
    }
    String::new()
}

pub struct Check<'a> {
    pub name: &'a str,
    pub label: &'a str,
    pub url: Option<&'a str>,
    pub outcome: Outcome,
    pub timing: String,
}
impl<'a> Check<'a> {
    pub fn new(check: &'a GitHubStatusCheck, now: u64) -> Self {
        match check {
            GitHubStatusCheck::CheckRun {
                name,
                status,
                conclusion,
                details_url,
                started_at,
                completed_at,
                ..
            } => {
                let label = conclusion
                    .as_deref()
                    .filter(|c| !c.is_empty())
                    .unwrap_or(status);
                let timing = started_at
                    .as_deref()
                    .and_then(timestamp)
                    .map(|start| {
                        duration(
                            completed_at
                                .as_deref()
                                .and_then(timestamp)
                                .unwrap_or(now)
                                .saturating_sub(start),
                        )
                    })
                    .unwrap_or_else(|| "—".into());
                Self {
                    name,
                    label,
                    url: details_url.as_deref(),
                    outcome: outcome(label, status.eq_ignore_ascii_case("completed")),
                    timing,
                }
            }
            GitHubStatusCheck::StatusContext {
                context,
                state,
                target_url,
                ..
            } => Self {
                name: context,
                label: state,
                url: target_url.as_deref(),
                outcome: outcome(state, true),
                timing: "—".into(),
            },
        }
    }
}

// GitHub timestamps are UTC RFC3339. Convert the civil date without local timezones.
fn timestamp(value: &str) -> Option<u64> {
    let date = value.get(..10)?;
    let parts: Vec<i64> = date
        .split('-')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let [mut y, m, d] = parts.as_slice() else {
        return None;
    };
    y -= i64::from(*m <= 2);
    let era = y.div_euclid(400);
    let year = y - era * 400;
    let month = *m + if *m > 2 { -3 } else { 9 };
    let days =
        era * 146097 + year * 365 + year / 4 - year / 100 + (153 * month + 2) / 5 + *d - 1 - 719468;
    let time: Vec<i64> = value
        .get(11..19)?
        .split(':')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let [h, m, s] = time.as_slice() else {
        return None;
    };
    u64::try_from((days * 86400 + h * 3600 + m * 60 + s) * 1000).ok()
}
