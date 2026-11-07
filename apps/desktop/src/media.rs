use crate::ui::Button;
use crate::{
    Workspace,
    live::{live, live_mut},
    panels::Screen,
    ui,
};
use caelum_core::application::{Attachment, Task, presentation::Media};
use rune::{El, Image, ImageCache, ScrollSpec, SurfaceFit, Waker, col, row};
use rune_kit::{ButtonVariant, Kit};
use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

type Decoded = HashMap<u64, (u64, Option<Result<Image, String>>)>;

pub struct State {
    cache: Arc<ImageCache>,
    decoded: Arc<Mutex<Decoded>>,
    changed: Arc<AtomicBool>,
    wake: Waker,
    pub selected: Option<(String, Media)>,
    pub zoom: Option<f32>,
    offset: std::rc::Rc<std::cell::Cell<[f32; 2]>>,
    drag_start: [f32; 2],
}
impl Default for State {
    fn default() -> Self {
        Self::new(Arc::new(|| {}))
    }
}
impl State {
    pub fn new(wake: Waker) -> Self {
        Self {
            cache: Arc::new(ImageCache::new(wake.clone())),
            decoded: Default::default(),
            changed: Default::default(),
            wake,
            selected: None,
            zoom: None,
            offset: Default::default(),
            drag_start: [0.0; 2],
        }
    }
    pub fn poll(&self) -> bool {
        self.changed.swap(false, Ordering::AcqRel)
    }
    fn image(&self, data: &str, fixture: bool) -> Result<Option<Image>, String> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hash);
        let id = hash.finish();
        let generation = rune::runtime::commit_generation();
        let mut decoded = self.decoded.lock().unwrap();
        if let Some((used, result)) = decoded.get_mut(&id) {
            *used = generation;
            return result.clone().transpose();
        }
        if decoded.len() >= 12
            && let Some(oldest) = decoded
                .iter()
                .min_by_key(|(_, (used, _))| used)
                .map(|(id, _)| *id)
        {
            decoded.remove(&oldest);
        }
        decoded.insert(id, (generation, None));
        drop(decoded);
        let cache = self.cache.clone();
        let target = self.decoded.clone();
        let changed = self.changed.clone();
        let wake = self.wake.clone();
        let data = data.to_owned();
        let load = move || {
            let result = Attachment::image_bytes(&data)
                .ok_or_else(|| "Unsupported image".into())
                .map(|(mime, bytes)| cache.load_mime(bytes, &mime, 900.0, 2.0));
            if let Some((_, value)) = target.lock().unwrap().get_mut(&id) {
                *value = Some(result);
            }
            changed.store(true, Ordering::Release);
            wake();
        };
        if fixture {
            load();
            self.decoded.lock().unwrap()[&id].1.clone().transpose()
        } else {
            std::thread::spawn(load);
            Ok(None)
        }
    }
}
fn data<'a>(s: &'a Workspace, project: &str, source: &'a str) -> Option<&'a str> {
    if source.starts_with("data:image/") {
        Some(source)
    } else {
        live(s)
            .app
            .services
            .images
            .get(&(project.into(), source.into()))
            .map(String::as_str)
    }
}
pub fn request(s: &mut Workspace, index: usize, positions: &[usize]) -> bool {
    let l = s.live.as_mut().unwrap();
    let project = l.app.sessions[index].scope.project.clone();
    let sources: Vec<_> = positions
        .iter()
        .filter_map(|i| l.app.sessions[index].conversation.entries.get(*i))
        .flat_map(|e| e.media.iter().map(|m| m.source.clone()))
        .collect();
    for source in sources {
        let task = Task::Image {
            project: project.clone(),
            source: source.clone(),
        };
        if !source.starts_with("data:")
            && !l
                .app
                .services
                .images
                .contains_key(&(project.clone(), source))
            && !l.app.services.busy(&task)
            && l.app.services.error(&task).is_none()
        {
            l.app.task(task);
        }
    }
    false
}
pub fn thumbnail(s: &Workspace, k: &Kit, project: &str, media: &Media) -> El {
    let Some(data) = data(s, project, &media.source) else {
        let task = Task::Image {
            project: project.into(),
            source: media.source.clone(),
        };
        return Button::new(if live(s).app.services.error(&task).is_some() {
            "Retry image"
        } else {
            "Loading image…"
        })
        .disabled(live(s).app.services.busy(&task))
        .on_press(move |s: &mut Workspace| live_mut(s).app.task(task.clone()))
        .build(k);
    };
    let image = match live(s).media.image(data, live(s).fixture) {
        Ok(Some(image)) => image,
        Ok(None) => return ui::skeleton("Loading image", 2, k),
        Err(error) => return ui::notice("Image unavailable", &error, k),
    };
    if live(s).fixture {
        let _ = image.wait();
    }
    if let Some(error) = image.error() {
        return ui::caption(error.to_string(), k);
    }
    let owner = project.to_owned();
    let media = media.clone();
    rune::el()
        .w_full()
        .h(170.0)
        .max_w(420.0)
        .radius(8.0)
        .border(1.0, k.c.border_base)
        .focus_ring(1.5, 0.0, k.c.border_focus)
        .pixels_fit(image.pixels(), SurfaceFit::Fit)
        .label(&media.name)
        .role("img")
        .focusable(true)
        .on_press(move |s: &mut Workspace| {
            let l = live_mut(s);
            l.media.selected = Some((owner.clone(), media.clone()));
            l.media.zoom = None;
            l.panels.overlay = Some(Screen::Image);
        })
}
pub fn view(s: &Workspace, k: &Kit) -> El {
    let Some((project, media)) = &live(s).media.selected else {
        return col();
    };
    let Some(data) = data(s, project, &media.source) else {
        return ui::caption("Image unavailable", k);
    };
    let image = match live(s).media.image(data, live(s).fixture) {
        Ok(Some(image)) => image,
        Ok(None) => return ui::skeleton("Loading image", 3, k),
        Err(error) => return ui::notice("Image unavailable", &error, k),
    };
    let [width, height] = image.size().unwrap_or([640, 400]);
    let canvas = rune::memo::window_layout("image-canvas").unwrap_or([0.0, 0.0, 640.0, 340.0]);
    let fit = (canvas[2] / width as f32)
        .min(canvas[3] / height as f32)
        .min(1.0);
    let zoom = live(s).media.zoom.unwrap_or(fit);
    let mut actions = row().w_full().center_y().gap(6.0);
    for (name, value) in [
        ("Fit", None),
        ("Actual size", Some(1.0)),
        ("−", Some((zoom / 1.4).max(0.1))),
        ("+", Some((zoom * 1.4).min(8.0))),
    ] {
        actions = actions.child(
            Button::new(name)
                .variant(ButtonVariant::Outline)
                .on_press(move |s: &mut Workspace| live_mut(s).media.zoom = value)
                .build(k),
        );
    }
    let offset = live(s).media.offset.clone();
    col()
        .w_full()
        .h_full()
        .min_h(0.0)
        .gap(8.0)
        .child(ui::caption(&media.name, k).truncate())
        .child(actions.child(ui::caption(
            format!("{:.0}% · {width} × {height}", zoom * 100.0),
            k,
        )))
        .child(
            col()
                .key("image-canvas")
                .label("Image canvas")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .radius(8.0)
                .fill(k.c.bg_deep)
                .border(1.0, k.c.border_base)
                .scroll_xy()
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .on_scroll_offset(move |pos| offset.set(pos.offset))
                .on_drag(|s: &mut Workspace, drag, tx| {
                    let m = &mut live_mut(s).media;
                    if drag.phase == rune::Phase::Down {
                        m.drag_start = m.offset.get();
                    }
                    let at = if drag.cancelled() {
                        m.drag_start
                    } else {
                        [
                            (m.drag_start[0] - drag.dx).max(0.0),
                            (m.drag_start[1] - drag.dy).max(0.0),
                        ]
                    };
                    tx.scroll_to("image-canvas", at[0], at[1]);
                })
                .items_center()
                .child(
                    rune::el()
                        .size(width as f32 * zoom, height as f32 * zoom)
                        .pixels(image.pixels())
                        .label(&media.name)
                        .role("img"),
                ),
        )
}
