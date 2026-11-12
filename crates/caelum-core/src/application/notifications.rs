use super::SessionKey;

/// A desktop effect. The originating session survives navigation and app restarts.
#[derive(Debug, Clone)]
pub struct Notification {
    pub title: String,
    pub body: String,
    pub target: Option<SessionKey>,
}
