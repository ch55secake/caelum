use super::Live;
use crate::{Workspace, remote::native};
use caelum_core::{
    application::{Runtime, Store},
    remote::native::Status,
};
use rune::Waker;

pub fn setup(s: &mut Workspace, wake: Waker) {
    if let Some(live) = &mut s.live
        && !live.evaluation
        && !live.fixture
    {
        live.start(native::store(live.native.as_ref()), wake);
    }
}

impl Live {
    pub(crate) fn start(&mut self, store: Store, wake: Waker) {
        let parent = Store::default_path();
        let lock = match &self.native {
            Some(n) => n.target.with_live_profile(&parent, || store.lock()),
            None => store.lock(),
        };
        match lock {
            Ok(lock) => self.store_lock = Some(lock),
            Err(error) => {
                if let Some(n) = self.native.as_mut() {
                    n.status = Status::Disconnected(error.clone());
                }
                self.app.persistence_error = Some(error);
                return;
            }
        }
        self.wake = Some(wake.clone());
        self.media = crate::media::State::new(wake.clone());
        self.runtime = Some(if let Some(n) = self.native.as_mut() {
            n.start(&parent, wake.clone());
            Runtime::native_desktop(n.target.clone(), parent, move || wake())
        } else {
            Runtime::new(move || wake())
        });
        self.app.enable_store(store.0);
    }
}

impl Drop for Live {
    fn drop(&mut self) {
        self.app.home.draft.text.clone_from(&self.home.editor.text);
        self.panels.remember(&mut self.app);
        for (i, editor) in self.drafts.iter().enumerate() {
            self.app.sessions[i].draft.clone_from(&editor.text);
        }
        drop(self.runtime.take());
        let saved = match self.native.take().map(|n| n.target) {
            Some(target) if self.store_lock.is_some() => {
                target.with_live_profile(&Store::default_path(), || self.app.flush())
            }
            Some(_) => Ok(()),
            None => self.app.flush(),
        };
        if let Err(error) = saved {
            self.updates.abort_restart();
            eprintln!("{error}");
        }
    }
}
