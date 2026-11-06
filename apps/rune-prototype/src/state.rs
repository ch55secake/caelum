use std::time::Duration;

use rune::{TextEdit, Tx};
use rune_kit::{
    diff::{self, DiffState},
    shell::PaneState,
};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sample {
    pub title: String,
    pub secondary_title: String,
    pub prompt: String,
    pub command: String,
    pub output: String,
    pub response: String,
    pub draft: String,
    pub patch: String,
}

pub struct Turn {
    pub prompt: String,
    pub response: String,
    pub tool: bool,
    pub expanded: bool,
    pub stopped: bool,
}

pub struct Session {
    pub title: String,
    pub draft: TextEdit,
    pub turns: Vec<Turn>,
    stream: Option<Stream>,
}

struct Stream {
    text: String,
    started: Option<f64>,
}

impl Session {
    fn new(title: String) -> Self {
        Self {
            title,
            draft: TextEdit::new(""),
            turns: Vec::new(),
            stream: None,
        }
    }

    pub fn working(&self) -> bool {
        self.stream.is_some()
    }

    pub fn stop(&mut self) {
        if self.stream.take().is_some()
            && let Some(turn) = self.turns.last_mut()
        {
            turn.stopped = true;
        }
    }
}

pub struct Workspace {
    pub sample: Sample,
    pub sessions: Vec<Session>,
    pub selected: usize,
    pub sidebar: PaneState,
    pub inspector: PaneState,
    pub diff: DiffState,
    pub dark: Option<bool>,
}

impl Workspace {
    pub fn new() -> Self {
        let sample: Sample =
            serde_json::from_str(include_str!("../../../docs/design/workspace.json"))
                .expect("valid shared workspace fixture");
        let mut first = Session::new(sample.title.clone());
        first.draft = TextEdit::new(sample.draft.clone());
        first.turns.push(Turn {
            prompt: sample.prompt.clone(),
            response: sample.response.clone(),
            tool: true,
            expanded: false,
            stopped: false,
        });
        let second = Session::new(sample.secondary_title.clone());
        let diff = DiffState::new(diff::parse(&sample.patch));
        Self {
            sample,
            sessions: vec![first, second],
            selected: 0,
            sidebar: PaneState::new(238.0).range(200.0, 360.0),
            inspector: PaneState::new(380.0).range(280.0, 560.0).collapsed(),
            diff,
            dark: None,
        }
    }

    pub fn active(&self) -> &Session {
        &self.sessions[self.selected]
    }

    pub fn active_mut(&mut self) -> &mut Session {
        &mut self.sessions[self.selected]
    }

    pub fn new_session(&mut self) {
        self.selected = self.sessions.len();
        let mut session = Session::new("New session".into());
        session.draft.focused = true;
        self.sessions.push(session);
    }

    pub fn send(&mut self, tx: &mut Tx) {
        let session = self.active_mut();
        let prompt = session.draft.text.trim().to_owned();
        if prompt.is_empty() || session.working() {
            return;
        }
        if session.turns.is_empty() {
            session.title = prompt
                .lines()
                .next()
                .unwrap_or(&prompt)
                .chars()
                .take(48)
                .collect();
        }
        session.turns.push(Turn {
            prompt,
            response: String::new(),
            tool: false,
            expanded: false,
            stopped: false,
        });
        session.stream = Some(Stream {
            text: "## Reviewing the details\n\nThis is a **simulated response** so you can try Rune’s interaction and rendering.\n\n- Resize the sidebar and changes inspector.\n- Switch sessions while this reply arrives; each draft stays with its session.\n- Select or copy text, then try a multiline message.\n\n```css\n.workspace {\n  --radius: 12px;\n  --space: 16px;\n  --motion: 160ms;\n}\n```\n\nOpen **Changes** to compare the sample patch.".into(),
            started: None,
        });
        session.draft.clear();
        session.draft.focused = true;
        tx.scroll_to_bottom(format!("transcript-{}", self.selected));
        tx.wake_after(Duration::from_millis(25));
    }
}

// The frame clock is virtual in headless runs, so the same interaction is reproducible.
// Advance every session, including those currently hidden behind another selection.
pub fn poll(s: &mut Workspace, now: f64, tx: &mut Tx) -> bool {
    let mut changed = false;
    for session in &mut s.sessions {
        let Some(stream) = &mut session.stream else {
            continue;
        };
        let start = *stream.started.get_or_insert(now);
        let count = ((now - start).max(0.0) * 90.0) as usize;
        let next: String = stream.text.chars().take(count).collect();
        let done = next.len() == stream.text.len();
        let turn = session
            .turns
            .last_mut()
            .expect("a stream belongs to a turn");
        changed |= turn.response != next || done;
        turn.response = next;
        if done {
            session.stream = None;
        } else {
            tx.wake_after(Duration::from_millis(25));
        }
    }
    changed
}
