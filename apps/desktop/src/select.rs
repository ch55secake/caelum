//! Shared native choice fields and anchored menus.
use rune::{El, Tx, col, icon, row, text};
use rune_kit::{Kit, Placement, ScrollBar, SelectAppearance};

type Handler = Box<dyn FnOnce(El) -> El>;
type Choose = Box<dyn Fn(&str, El) -> El>;
type Dismiss = Box<dyn FnOnce(El, &mut Vec<El>) -> El>;

pub struct Select {
    key: String,
    options: Vec<String>,
    current: Option<String>,
    placeholder: String,
    label: String,
    display: Option<String>,
    prominent: bool,
    indicator: &'static rune::Icon,
    field: bool,
    width: f32,
    open: bool,
    disabled: bool,
    placement: Placement,
    toggle: Option<Handler>,
    choose: Option<Choose>,
    dismiss: Option<Dismiss>,
    search: Option<El>,
}

impl Select {
    pub fn new<T: AsRef<str>>(key: impl Into<String>, options: &[T]) -> Self {
        Self {
            key: key.into(),
            options: options.iter().map(|v| v.as_ref().into()).collect(),
            current: None,
            placeholder: String::new(),
            label: String::new(),
            display: None,
            prominent: false,
            indicator: &crate::ui::CHOICE,
            field: false,
            width: 260.0,
            open: false,
            disabled: false,
            placement: Placement::BottomStart,
            toggle: None,
            choose: None,
            dismiss: None,
            search: None,
        }
    }
    pub fn current(mut self, value: Option<&str>) -> Self {
        self.current = value.map(str::to_owned);
        self
    }
    pub fn placeholder(mut self, value: impl Into<String>) -> Self {
        self.placeholder = value.into();
        self
    }
    pub fn label(mut self, value: impl Into<String>) -> Self {
        self.label = value.into();
        self
    }
    pub fn display(mut self, value: impl Into<String>) -> Self {
        self.display = Some(value.into());
        self
    }
    pub fn prominent(mut self) -> Self {
        self.prominent = true;
        self
    }
    pub fn indicator(mut self, value: &'static rune::Icon) -> Self {
        self.indicator = value;
        self
    }
    pub fn appearance(mut self, value: SelectAppearance) -> Self {
        self.field = value == SelectAppearance::Field;
        self
    }
    pub fn width(mut self, value: f32) -> Self {
        self.width = value;
        self
    }
    pub fn open(mut self, value: bool) -> Self {
        self.open = value;
        self
    }
    pub fn disabled(mut self, value: bool) -> Self {
        self.disabled = value;
        self
    }
    pub fn placement(mut self, value: Placement) -> Self {
        self.placement = value;
        self
    }
    pub fn search(mut self, field: El) -> Self {
        self.search = Some(field);
        self
    }
    pub fn on_toggle<S: 'static>(mut self, f: impl Fn(&mut S) + 'static) -> Self {
        self.toggle = Some(Box::new(move |el| el.on_press(f)));
        self
    }
    pub fn on_close<S: 'static>(mut self, f: impl Fn(&mut S) + Clone + 'static) -> Self {
        let key = format!("{}-scrim", self.key);
        self.dismiss = Some(Box::new(move |el, layers| {
            layers.push(
                rune::el()
                    .key(key)
                    .absolute(0.0, 0.0)
                    .w_full()
                    .h_full()
                    .on_press(f.clone()),
            );
            el.modal(true, f)
        }));
        self
    }
    pub fn on_choose<S: 'static>(
        mut self,
        f: impl Fn(&mut S, &str, &mut Tx) + Clone + 'static,
    ) -> Self {
        self.choose = Some(Box::new(move |value, el| {
            let (f, value) = (f.clone(), value.to_owned());
            el.on_press_tx(move |s: &mut S, tx| f(s, &value, tx))
        }));
        self
    }
    fn field(&mut self, k: &Kit) -> El {
        let value = self.current.as_deref().unwrap_or(&self.placeholder);
        let display = self.display.as_deref().unwrap_or(value);
        let mut field = k.interactive(
            row()
                .gap(8.0)
                .center_y()
                .px(10.0)
                .h(if self.field { 34.0 } else { 28.0 })
                .radius(6.0)
                .fill(if self.open {
                    k.c.hover
                } else if self.field {
                    k.c.layer_02
                } else {
                    k.c.bg_base
                })
                .children((!display.is_empty()).then(|| {
                    text(display)
                        .font(if self.prominent {
                            k.ty.ui_medium
                        } else {
                            k.ty.ui
                        })
                        .color(if self.prominent || self.field {
                            k.c.text_base
                        } else {
                            k.c.text_muted
                        })
                        .truncate()
                        .min_w(0.0)
                        .grow(1.0)
                })),
        );
        if self.field {
            field = field
                .w(self.width)
                .border(
                    1.0,
                    if self.open {
                        k.c.border_focus
                    } else {
                        k.c.border_base
                    },
                )
                .child(icon(self.indicator, 13.0).color(k.c.icon_muted));
        } else {
            field = field
                .w(
                    (rune::text::measure(display, &[], k.ty.ui_medium, None).0 + 20.0)
                        .clamp(36.0, 220.0),
                )
                .border(
                    1.0,
                    if self.prominent {
                        k.c.border_strong
                    } else {
                        k.c.border_base
                    },
                );
        }
        field = field
            .key(format!("{}-trigger", self.key))
            .role("combobox")
            .label(&self.label)
            .a11y_value(value)
            .expanded(self.open)
            .disabled(self.disabled)
            .focusable(!self.disabled)
            .focus_ring(1.5, 1.0, k.c.border_focus)
            .opacity(if self.disabled { 0.45 } else { 1.0 });
        if let Some(f) = self.toggle.take() {
            field = f(field);
        }
        field
    }
    pub fn trigger(mut self, k: &Kit) -> El {
        self.field(k)
    }
    pub fn build(mut self, k: &Kit, layers: &mut Vec<El>) -> El {
        let trigger = self.field(k);
        if !self.open || self.disabled {
            return trigger;
        }
        let anchor = format!("{}-trigger", self.key);
        let width = rune::memo::window_layout(&anchor)
            .map_or(self.width, |r| r[2])
            .clamp(200.0, 340.0);
        let mut list = col()
            .w_full()
            .gap(2.0)
            .focus_group()
            .role("listbox")
            .label(&self.label);
        for (i, value) in self.options.iter().enumerate() {
            let selected = self.current.as_ref() == Some(value);
            let mut item = k
                .interactive(
                    row()
                        .w_full()
                        .h(32.0)
                        .px(10.0)
                        .gap(12.0)
                        .center_y()
                        .radius(6.0)
                        .fill(if selected {
                            k.c.hover
                        } else {
                            rune::Color::TRANSPARENT
                        })
                        .child(
                            text(value)
                                .font(k.ty.ui)
                                .color(k.c.text_base)
                                .truncate()
                                .min_w(0.0)
                                .grow(1.0),
                        )
                        .child(
                            icon(k.glyph.check_small, 14.0)
                                .color(k.c.text_accent)
                                .opacity(if selected { 1.0 } else { 0.0 }),
                        ),
                )
                .key(format!("option{i}"))
                .role("option")
                .label(value)
                .focusable(true)
                .focus_selected(selected)
                .focus_highlight(k.c.hover)
                .focus_ring(1.0, 0.0, k.c.border_focus);
            if let Some(f) = &self.choose {
                item = f(value, item);
            }
            list = list.child(item);
        }
        if self.options.is_empty() {
            list = list.child(crate::ui::caption("No matching choices", k).padding(10.0));
        }
        let mut content = col()
            .key(format!("{}-content", self.key))
            .label(&self.label)
            .w(width)
            .padding(6.0)
            .gap(6.0)
            .focus_return(&anchor)
            .children(self.search)
            .child(
                col()
                    .w_full()
                    .max_h(238.0)
                    .min_h(0.0)
                    .scroll(rune::ScrollSpec {
                        stick_bottom: false,
                    })
                    .child(list)
                    .scroll_bar(k),
            );
        if let Some(f) = self.dismiss {
            content = f(content, layers);
        }
        layers.push(rune_kit::popover::anchored(
            k,
            rune_kit::popover::raised(k, content, 10.0),
            &anchor,
            self.placement,
            6.0,
        ));
        trigger
    }
}
