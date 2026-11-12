use crate::{Workspace, evaluation, live, on_key, state, view};
use rune::AppSpec;

pub(super) fn spec() -> AppSpec<Workspace> {
    AppSpec {
        min_size: Some((800.0, 620.0)),
        families: ("system-ui", "ui-monospace"),
        poll: Some(state::poll),
        setup: Some(live::setup),
        on_key: Some(on_key),
        scenes: evaluation::SCENES,
        ..AppSpec::new("Caelum", (1280.0, 1000.0), Workspace::new, view)
    }
}
