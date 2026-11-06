use rune::{Color, TextStyle, color::hex};
use rune_kit::{Kit, MarkdownStyle};

// Caelum's default palette, mapped to Rune's semantic component tokens.
pub fn kit(dark: bool) -> Kit {
    let mut k = Kit::neutral(dark);
    let color = |light, dark_value| hex(if dark { dark_value } else { light }).unwrap();
    let bg = color("#fafafc", "#0b0c0e");
    let sidebar = color("#f0f0f5", "#101114");
    let panel = color("#ffffff", "#17181c");
    let border = color("#dcdde5", "#25262d");
    let text = color("#252630", "#e7e8ec");
    let muted = color("#656776", "#9698a3");
    let accent = color("#6c56b0", "#a39af6");
    k.c.page = bg;
    k.c.bg_base = bg;
    k.c.bg_deep = sidebar;
    k.c.layer_01 = sidebar;
    k.c.layer_02 = panel;
    k.c.layer_03 = color("#e9e9f1", "#1e1f25");
    k.c.bg_button_neutral = panel;
    k.c.bg_accent = hex("#6659b0").unwrap();
    k.c.bg_icon_button_contrast = k.c.bg_accent;
    k.c.submit_highlight = Color::TRANSPARENT;
    k.c.submit_fg = Color::WHITE;
    k.c.submit_icon = Color::WHITE;
    k.c.text_base = text;
    k.c.text_strong = text;
    k.c.text_muted = muted;
    k.c.text_faint = muted;
    k.c.text_accent = accent;
    k.c.icon_base = text;
    k.c.icon_muted = muted;
    k.c.icon_faint = muted;
    k.c.icon_accent = accent;
    k.c.border_base = border;
    k.c.border_muted = border;
    k.c.border_weak = border;
    k.c.border_strong = border;
    k.c.border_focus = accent;
    k.c.border_active = accent;
    k.c.surface_base = bg;
    k.c.surface_weak = sidebar;
    k.c.surface_raised = panel;
    k.c.surface_raised_base = panel;
    k.c.success.icon = color("#337348", "#91c69b");
    k.c.danger.icon = color("#b53a51", "#efa4ab");
    k.radius.md = 6.0;
    k.radius.lg = 14.0;
    k.ty.body = TextStyle::new(15.0, 400.0).lh(26.0);
    k.ty.body_medium = TextStyle::new(15.0, 550.0).lh(26.0);
    k
}

pub fn markdown(k: &Kit) -> MarkdownStyle {
    MarkdownStyle::from_kit(k)
        .body_font(k.ty.body)
        .heading_font(2, TextStyle::new(18.0, 600.0).lh(27.0))
        .heading_font(3, TextStyle::new(15.0, 600.0).lh(24.0))
        .code_block_font(TextStyle {
            size: 12.0,
            line_height: 22.0,
            ..k.ty.mono
        })
        .code_block_padding(16.0, 16.0)
        .code_block(k.c.bg_deep, k.c.border_base)
        .list(24.0, 4.0)
}
