use super::{attachments, controls};
use crate::{
    Workspace,
    context::EditMenu,
    live::{live, live_mut},
    on_key, ui,
};
use rune::{Color, El, SurfaceFit, TextStyle, col, el, icon, row, text, text_input};
use rune_kit::{ButtonVariant, Kit};

pub fn view(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let l = live(s);
    let size = rune::memo::window_layout("main-pane").unwrap_or([0.0, 0.0, 1000.0, 900.0]);
    let compact = size[3] < 680.0;
    let background = el()
        .key("home-background")
        .pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
        .fill(rune::color::hex(if k.dark { "#08090b" } else { "#f8f8f4" }).unwrap())
        .a11y_hidden();
    let background = if let Some(artwork) = &l.home.background {
        let image = if k.dark {
            &artwork.dark
        } else {
            &artwork.light
        };
        background.pixels_fit(image.pixels(), SurfaceFit::Fill)
    } else {
        background
    };
    let content = col()
        .key("home-content")
        .w_full()
        .max_w(688.0)
        .px(if size[2] < 540.0 { 20.0 } else { 24.0 })
        .gap(22.0)
        .items_center()
        .child(
            icon(&crate::ORBIT, if compact { 32.0 } else { 42.0 })
                .color(k.c.text_base)
                .a11y_hidden(),
        )
        .child(
            text("A little room to think")
                .key("home-heading")
                .font(TextStyle::new(if compact { 25.0 } else { 30.0 }, 500.0).lh(38.0))
                .color(k.c.text_base)
                .wrap(),
        )
        .child(composer(s, k, layers))
        .child(controls::project(s, k))
        .children((l.app.home.opening()).then(|| ui::caption("Opening project…", k)))
        .children(
            l.app
                .home
                .error
                .as_ref()
                .or(l.app.persistence_error.as_ref())
                .or(l.app.error.as_ref())
                .map(|error| ui::caption(error, k).color(k.c.danger.fg).w_full()),
        );
    col()
        .key("home")
        .w_full()
        .h_full()
        .min_h(0.0)
        .clip_rounded(0.0)
        .child(background)
        .children(k.dark.then(|| {
            el().pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0))
                .gradient(Color::BLACK.alpha(0.06), Color::BLACK.alpha(0.55))
                .a11y_hidden()
        }))
        .child(
            col()
                .key("home-scroll")
                .w_full()
                .h_full()
                .scroll(rune::ScrollSpec {
                    stick_bottom: false,
                })
                .child(
                    col()
                        .w_full()
                        .min_h(size[3])
                        .py(32.0)
                        .items_center()
                        .justify_center()
                        .child(content),
                ),
        )
}

fn composer(s: &Workspace, k: &Kit, layers: &mut Vec<El>) -> El {
    let l = live(s);
    let position = l.home.position.clone();
    let mounted = l.home.mounted;
    let input = text_input(&l.home.editor, TextStyle::new(14.0, 400.0).lh(23.0))
        .label("Home message")
        .placeholder("Describe a task or ask a question…", k.c.text_muted)
        .help("Enter to start a new session · Shift+Enter for a new line")
        .color(k.c.text_base)
        .selection_color(k.selection())
        .w_full()
        .edit_menu_changed(
            "home-draft",
            |s| Some(&mut live_mut(s).home.editor),
            super::changed,
        )
        .on_input_key(on_key)
        .on_submit(super::send)
        .on_input_paste(|s: &mut Workspace, content, _| {
            let Some(image) = content.image() else {
                return false;
            };
            live_mut(s)
                .app
                .home_attach_image("Pasted image.png".into(), image.png().to_vec());
            true
        })
        .on_drop_files(|s: &mut Workspace, paths, _| attachments::drop_files(s, paths));
    col()
        .key("home-composer")
        .role("form")
        .label("Start a new session")
        .w_full()
        .padding(14.0)
        .gap(12.0)
        .radius(16.0)
        .fill(k.c.layer_02)
        .border(1.0, k.c.border_strong)
        .child(attachments::view(s, k))
        .child(
            rune::auto_grow(input, 2, 8)
                .key("home-editor")
                .min_h(48.0)
                .max_h(184.0)
                .px(4.0)
                .py(4.0)
                .on_scroll_offset(move |at| {
                    if mounted {
                        position.set(at);
                    }
                }),
        )
        .child(
            row()
                .w_full()
                .flex_wrap()
                .center_y()
                .gap(6.0)
                .child(
                    crate::quiet_button(&crate::PLUS, "Attach a file")
                        .disabled(!l.app.home_project_available())
                        .on_press_tx(attachments::choose_file)
                        .build(k)
                        .key("home-attach"),
                )
                .child(controls::provider(s, k, layers))
                .child(controls::model(s, k))
                .children(controls::effort(s, k, layers))
                .child(el().grow(1.0))
                .child(
                    ui::Button::new("Send")
                        .icon(&crate::SEND)
                        .variant(ButtonVariant::Contrast)
                        .disabled(
                            l.app.home.opening()
                                || l.app.home.attaching > 0
                                || (l.home.editor.text.trim().is_empty()
                                    && l.app.home.draft.attachments.is_empty()),
                        )
                        .on_press_tx(super::send)
                        .build(k)
                        .key("home-send")
                        .label("Start session"),
                ),
        )
}
