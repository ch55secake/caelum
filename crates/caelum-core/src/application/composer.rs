use serde::{Deserialize, Serialize};

use super::Options;

/// Immutable context captured when the user attaches it, never re-read on send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
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
            id,
            path,
            content,
            start_line: range.map(|v| v.0),
            end_line: range.map(|v| v.1),
            truncated,
        }
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
    format!(
        "{text}\n\n<caelum-file-context>\n{}\n</caelum-file-context>",
        serde_json::json!({ "version": 1, "files": files })
    )
}

pub fn split_context(prompt: &str) -> (&str, Vec<Attachment>) {
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
