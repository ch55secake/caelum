use serde::{Deserialize, Serialize};

use super::Options;

pub(super) fn command_files(prompt: &str) -> Vec<caelum_protocol::OpenCodeCommandFile> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    split_context(prompt)
        .1
        .iter()
        .filter(|a| matches!(a.kind, AttachmentKind::File | AttachmentKind::Image))
        .map(|a| caelum_protocol::OpenCodeCommandFile {
            uri: if a.kind == AttachmentKind::Image {
                a.content.clone()
            } else {
                format!(
                    "data:text/plain;base64,{}",
                    STANDARD.encode(a.content.as_bytes())
                )
            },
            name: a.path.clone(),
            description: format!(
                "{}{}{}",
                a.start_line
                    .zip(a.end_line)
                    .map(|(start, end)| format!("Lines {start}–{end} of "))
                    .unwrap_or_else(|| "Snapshot of ".into()),
                a.path,
                if a.truncated { " (truncated)" } else { "" }
            ),
        })
        .collect()
}

/// Immutable context captured when the user attaches it, never re-read on send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    #[serde(default)]
    pub kind: AttachmentKind,
    pub id: String,
    pub path: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,
    pub truncated: bool,
}

impl Attachment {
    pub fn new(
        path: String,
        mut content: String,
        range: Option<(usize, usize)>,
        truncated: bool,
    ) -> Self {
        let mut end = content.len().min(64 * 1024);
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        let truncated = truncated || end < content.len();
        content.truncate(end);
        let id = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        Self {
            kind: AttachmentKind::File,
            id,
            path,
            content,
            start_line: range.map(|v| v.0),
            end_line: range.map(|v| v.1),
            truncated,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AttachmentKind {
    #[default]
    File,
    PullRequest,
    Image,
}

impl Attachment {
    pub fn image(path: String, bytes: &[u8]) -> Result<Self, String> {
        let content = crate::images::data_url(bytes)?;
        let mut attachment = Self::new(path, String::new(), None, false);
        attachment.kind = AttachmentKind::Image;
        attachment.content = content;
        Ok(attachment)
    }
    pub fn image_bytes(source: &str) -> Option<(String, Vec<u8>)> {
        use base64::{engine::general_purpose::STANDARD, Engine};
        let (mime, data) = source.strip_prefix("data:")?.split_once(";base64,")?;
        if !mime.starts_with("image/") || data.len() > 14 * 1024 * 1024 {
            return None;
        }
        Some((mime.into(), STANDARD.decode(data).ok()?))
    }
    pub fn pull_request(details: &caelum_protocol::GitHubPullRequestDetails) -> Self {
        let mut attachment = Self::new(
            details.pull_request.url.clone(),
            serde_json::to_string(details).expect("PR details"),
            None,
            false,
        );
        attachment.kind = AttachmentKind::PullRequest;
        attachment
    }
    pub fn label(&self) -> String {
        if self.kind == AttachmentKind::PullRequest {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&self.content) {
                return format!(
                    "#{} {}",
                    value["number"],
                    value["title"].as_str().unwrap_or("Pull request")
                );
            }
        }
        self.path.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionKind {
    Command,
    File,
    PullRequest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub kind: CompletionKind,
    pub start: usize,
    pub end: usize,
    pub query: String,
}

impl Completion {
    pub fn at(text: &str, anchor: usize, caret: usize) -> Option<Self> {
        if anchor != caret || !text.is_char_boundary(caret) {
            return None;
        }
        let prefix = text.get(..caret)?;
        let start = prefix
            .rfind(char::is_whitespace)
            .map_or(0, |i| i + prefix[i..].chars().next().unwrap().len_utf8());
        let word = &prefix[start..];
        let kind = match word.as_bytes().first()? {
            b'/' if start == 0 && !word.starts_with("//") => CompletionKind::Command,
            b'@' => CompletionKind::File,
            b'#' => CompletionKind::PullRequest,
            _ => return None,
        };
        let query = word.get(1..)?;
        if query.contains(['@', '#', '`', '"']) {
            return None;
        }
        let end = text[caret..]
            .find(|c: char| c.is_whitespace() || matches!(c, '@' | '#' | '`' | '"'))
            .map_or(text.len(), |n| caret + n);
        Some(Self {
            kind,
            start,
            end,
            query: query.into(),
        })
    }
    pub fn replace(&self, draft: &str, value: &str) -> (String, usize) {
        let suffix = &draft[self.end..];
        let space = if suffix.starts_with(char::is_whitespace) {
            ""
        } else {
            " "
        };
        (
            format!("{}{value}{space}{suffix}", &draft[..self.start]),
            self.start + value.len() + space.len(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueuedPrompt {
    pub text: String,
    pub options: Options,
}

pub(super) fn with_context(text: &str, files: &[Attachment]) -> String {
    if files.is_empty() {
        return text.into();
    }
    let mut prompt = text.to_owned();
    for (kind, tag, key) in [
        (AttachmentKind::File, "caelum-file-context", "files"),
        (
            AttachmentKind::PullRequest,
            "caelum-pull-request-context",
            "pullRequests",
        ),
        (AttachmentKind::Image, "caelum-image-context", "images"),
    ] {
        let values: Vec<_> = files.iter().filter(|f| f.kind == kind).collect();
        if !values.is_empty() {
            prompt.push_str(&format!(
                "\n\n<{tag}>\n{}\n</{tag}>",
                serde_json::json!({ "version":1, key:values })
            ));
        }
    }
    prompt
}

pub fn split_context(prompt: &str) -> (&str, Vec<Attachment>) {
    if let Some((text, suffix)) = prompt.rsplit_once("\n\n<caelum-image-context>\n") {
        if let Some(json) = suffix.strip_suffix("\n</caelum-image-context>") {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                if value["version"] == 1 {
                    if let Ok(images) =
                        serde_json::from_value::<Vec<Attachment>>(value["images"].clone())
                    {
                        if !images.is_empty()
                            && images.len() <= 16
                            && images.iter().all(|a| {
                                a.kind == AttachmentKind::Image
                                    && a.content.len() <= 14 * 1024 * 1024
                                    && a.content.starts_with("data:image/")
                            })
                        {
                            let (text, mut files) = split_context(text);
                            files.extend(images);
                            return (text, files);
                        }
                    }
                }
            }
        }
    }
    if let Some((text, suffix)) = prompt.rsplit_once("\n\n<caelum-pull-request-context>\n") {
        if let Some(json) = suffix.strip_suffix("\n</caelum-pull-request-context>") {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                if value["version"] == 1 {
                    if let Ok(prs) =
                        serde_json::from_value::<Vec<Attachment>>(value["pullRequests"].clone())
                    {
                        if !prs.is_empty()
                            && prs.len() <= 16
                            && prs.iter().all(|p| {
                                p.kind == AttachmentKind::PullRequest
                                    && p.content.len() <= 64 * 1024
                            })
                        {
                            let (text, mut files) = split_context(text);
                            files.extend(prs);
                            return (text, files);
                        }
                    }
                }
            }
        }
    }
    let Some((text, suffix)) = prompt.rsplit_once("\n\n<caelum-file-context>\n") else {
        return (prompt, Vec::new());
    };
    let Some(json) = suffix.strip_suffix("\n</caelum-file-context>") else {
        return (prompt, Vec::new());
    };
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(value) => value,
        Err(_) => return (prompt, Vec::new()),
    };
    if value["version"] != 1 {
        return (prompt, Vec::new());
    }
    let files: Vec<Attachment> = match serde_json::from_value(value["files"].clone()) {
        Ok(files) => files,
        Err(_) => return (prompt, Vec::new()),
    };
    if files.is_empty()
        || files.len() > 16
        || files.iter().any(|f| {
            f.path.is_empty()
                || f.content.len() > 64 * 1024
                || f.start_line
                    .zip(f.end_line)
                    .is_some_and(|(start, end)| start == 0 || end < start)
        })
    {
        return (prompt, Vec::new());
    }
    (text, files)
}

pub(crate) fn provider_prompt(prompt: &str) -> (String, Vec<Attachment>) {
    let (text, attachments) = split_context(prompt);
    if !attachments.iter().any(|a| a.kind == AttachmentKind::Image) {
        return (prompt.into(), Vec::new());
    }
    let (images, context): (Vec<_>, Vec<_>) = attachments
        .into_iter()
        .partition(|a| a.kind == AttachmentKind::Image);
    (with_context(text, &context), images)
}

pub(super) fn command_text(prompt: &str) -> String {
    let (text, attachments) = split_context(prompt);
    with_context(
        text,
        &attachments
            .into_iter()
            .filter(|a| a.kind == AttachmentKind::PullRequest)
            .collect::<Vec<_>>(),
    )
}
