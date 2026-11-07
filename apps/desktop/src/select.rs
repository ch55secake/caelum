//! Shared native choice fields and anchored menus.
use rune::{El, Tx, col, icon, row, text};
use rune_kit::{Kit, Placement, ScrollBar, SelectAppearance};

type Handler = Box<dyn FnOnce(El) -> El>;
type Choose = Box<dyn Fn(&str, El) -> El>;
type Dismiss = Box<dyn FnOnce(El, &mut Vec<El>) -> El>;

pub struct Choice {
    value: String,
    label: String,
    detail: Option<String>,
}

impl Choice {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            detail: None,
        }
    }
    pub fn detail(mut self, value: impl Into<String>) -> Self {
        self.detail = Some(value.into());
        self
    }
}

pub struct Select {
    key: String,
    options: Vec<Choice>,
    current: Option<String>,
    placeholder: String,
    label: String,
    display: Option<String>,
    prominent: bool,
    icon: Option<&'static rune::Icon>,
    menu_description: Option<String>,
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
            options: options
                .iter()
                .map(|v| Choice::new(v.as_ref(), v.as_ref()))
                .collect(),
            current: None,
            placeholder: String::new(),
            label: String::new(),
            display: None,
            prominent: false,
            icon: None,
            menu_description: None,
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
    pub fn choices(key: impl Into<String>, options: Vec<Choice>) -> Self {
        let mut select = Self::new::<&str>(key, &[]);
        select.options = options;
        select
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
    pub fn icon(mut self, value: &'static rune::Icon) -> Self {
        self.icon = Some(value);
        self
    }
    pub fn menu_description(mut self, value: impl Into<String>) -> Self {
        self.menu_description = Some(value.into());
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
        let value = self
            .options
            .iter()
            .find(|o| o.value == value)
            .map_or(value, |o| o.label.as_str());
        let display = self.display.as_deref().unwrap_or(value);
        let mut field = k.interactive(
            row()
                .gap(if self.field { 8.0 } else { 4.0 })
                .center_y()
                .px(if self.field { 10.0 } else { 6.0 })
                .h(if self.field { 34.0 } else { 28.0 })
                .radius(6.0)
                .fill(if self.open {
                    k.c.hover
                } else if self.field {
                    k.c.layer_02
                } else {
                    rune::Color::TRANSPARENT
                })
                .children(self.icon.map(|glyph| {
                    icon(glyph, 14.0)
                        .key(format!("{}-icon", self.key))
                        .color(k.c.icon_muted)
                        .a11y_hidden()
                }))
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
                .w((rune::text::measure(display, &[], k.ty.ui_medium, None).0
                    + 26.0
                    + if self.icon.is_some() { 18.0 } else { 0.0 })
                .clamp(36.0, 220.0))
                .child(icon(&rune_kit::glyphs::CHEVRON_DOWN, 10.0).color(k.c.icon_muted));
        }
        field = field
            .key(format!("{}-trigger", self.key))
            .role("combobox")
            .label(&self.label)
            .help(format!("{}: {value}", self.label))
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
        let width = if self.menu_description.is_some() {
            self.width
        } else {
            rune::memo::window_layout(&anchor).map_or(self.width, |r| r[2])
        }
        .clamp(200.0, 340.0);
        let mut list = col()
            .w_full()
            .gap(2.0)
            .focus_group()
            .role("listbox")
            .label(&self.label);
        for (i, choice) in self.options.iter().enumerate() {
            let selected = self.current.as_ref() == Some(&choice.value);
            let mut item = k
                .interactive(
                    row()
                        .w_full()
                        .min_h(if self.menu_description.is_some() {
                            36.0
                        } else {
                            32.0
                        })
                        .py(if choice.detail.is_some() { 7.0 } else { 0.0 })
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
                            col()
                                .min_w(0.0)
                                .grow(1.0)
                                .child(
                                    text(&choice.label)
                                        .font(if selected { k.ty.ui_medium } else { k.ty.ui })
                                        .color(k.c.text_base)
                                        .truncate(),
                                )
                                .children(choice.detail.as_ref().map(|detail| {
                                    text(detail).font(k.ty.small).color(k.c.text_muted)
                                })),
                        )
                        .child(
                            icon(k.glyph.check_small, 14.0)
                                .color(k.c.text_accent)
                                .opacity(if selected { 1.0 } else { 0.0 }),
                        ),
                )
                .key(format!("option{i}"))
                .role("option")
                .label(&choice.label)
                .focusable(true)
                .focus_selected(selected)
                .focus_highlight(k.c.hover)
                .focus_ring(1.0, 0.0, k.c.border_focus);
            if let Some(detail) = &choice.detail {
                item = item.description(detail);
            }
            if let Some(f) = &self.choose {
                item = f(&choice.value, item);
            }
            list = list.child(item);
        }
        if self.options.is_empty() {
            list = list.child(crate::ui::caption("No matching choices", k).padding(10.0));
        }
        let mut content =
            col()
                .key(format!("{}-content", self.key))
                .label(&self.label)
                .w(width)
                .padding(6.0)
                .gap(6.0)
                .focus_return(&anchor)
                .children(self.menu_description.as_ref().map(|description| {
                    col()
                        .w_full()
                        .gap(3.0)
                        .px(8.0)
                        .pt(6.0)
                        .pb(4.0)
                        .child(
                            row()
                                .gap(7.0)
                                .center_y()
                                .children(self.icon.map(|glyph| {
                                    icon(glyph, 14.0).color(k.c.icon_muted).a11y_hidden()
                                }))
                                .child(text(&self.label).font(k.ty.ui_medium).color(k.c.text_base)),
                        )
                        .child(text(description).font(k.ty.small).color(k.c.text_muted))
                        .child(crate::ui::divider(k).mt(7.0))
                }))
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
