use rune::{Color, TextStyle, color::hex};
use rune_kit::{Kit, MarkdownStyle};

// Caelum's default palette, mapped to Rune's semantic component tokens.
pub fn kit(dark: bool) -> Kit {
    appearance(dark, "caelum", "violet")
}

pub const FAMILIES: [(&str, &str); 4] = [
    ("caelum", "Caelum"),
    ("catppuccin", "Catppuccin"),
    ("nord", "Nord"),
    ("tokyo-night", "Tokyo Night"),
];
pub const ACCENTS: [&str; 7] = ["theme", "violet", "blue", "teal", "green", "amber", "rose"];

pub fn appearance(dark: bool, family: &str, accent_name: &str) -> Kit {
    let mut k = Kit::neutral(dark);
    let color = |light, dark_value| hex(if dark { dark_value } else { light }).unwrap();
    // Background, sidebar, panel, hover, border, text, muted, accent, solid, success, danger.
    let values = match (family, dark) {
        ("catppuccin", true) => [
            "#1e1e2e", "#181825", "#313244", "#45475a", "#45475a", "#cdd6f4", "#a6adc8", "#cba6f7",
            "#6c468f", "#a6e3a1", "#f38ba8",
        ],
        ("catppuccin", false) => [
            "#eff1f5", "#e6e9ef", "#ffffff", "#dce0e8", "#bcc0cc", "#4c4f69", "#6c6f85", "#8839ef",
            "#8839ef", "#287e1b", "#b3214d",
        ],
        ("nord", true) => [
            "#2e3440", "#272c36", "#3b4252", "#434c5e", "#4c566a", "#eceff4", "#a7b0c0", "#88c0d0",
            "#376778", "#a3be8c", "#e0949c",
        ],
        ("nord", false) => [
            "#eceff4", "#e5e9f0", "#ffffff", "#d8dee9", "#bac4d4", "#2e3440", "#5b667a", "#3b7189",
            "#3b7189", "#466834", "#9e3b4e",
        ],
        ("tokyo-night", true) => [
            "#1a1b26", "#16161e", "#24283b", "#292e42", "#414868", "#c0caf5", "#9aa5ce", "#7aa2f7",
            "#3d59a1", "#9ece6a", "#f7768e",
        ],
        ("tokyo-night", false) => [
            "#e1e2e7", "#d5d6db", "#f0f0f4", "#cbd0df", "#b4bdd2", "#343b58", "#526078", "#2e5f9d",
            "#34548a", "#385f0d", "#9f2240",
        ],
        (_, true) => [
            "#0b0c0e", "#101114", "#141519", "#1e1f25", "#25262d", "#e7e8ec", "#9698a3", "#a39af6",
            "#6659b0", "#89bc9b", "#efa4ab",
        ],
        (_, false) => [
            "#fafafc", "#f0f0f5", "#ffffff", "#e9e9f1", "#dcdde5", "#252630", "#656776", "#6c56b0",
            "#6659b0", "#337348", "#b53a51",
        ],
    };
    let [
        bg,
        sidebar,
        panel,
        hover,
        border,
        text,
        muted,
        mut accent,
        mut solid,
        success,
        danger,
    ] = values.map(|value| hex(value).unwrap());
    if let Some((light, dark, fill)) = match accent_name {
        "violet" => Some(("#6c56b0", "#a39af6", "#6659b0")),
        "blue" => Some(("#3568b0", "#82b4f4", "#3568b0")),
        "teal" => Some(("#216f68", "#72c7bf", "#23776f")),
        "green" => Some(("#406f36", "#91c788", "#487b3d")),
        "amber" => Some(("#875819", "#e4b56e", "#94621c")),
        "rose" => Some(("#a54163", "#e79ab0", "#ad476a")),
        _ => None,
    } {
        accent = color(light, dark);
        solid = hex(fill).unwrap();
    }
    k.c.page = bg;
    k.c.bg_base = bg;
    k.c.bg_deep = sidebar;
    k.c.layer_01 = panel;
    k.c.layer_02 = panel;
    k.c.layer_03 = hover;
    k.c.layer_04 = border;
    k.c.hover = k.alpha(text, 0.05);
    k.c.pressed = k.alpha(accent, 0.12);
    k.c.bg_button_neutral = panel;
    k.c.bg_accent = solid;
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
    k.c.border_strong = if family == "caelum" {
        color("#cfd0dc", "#303139")
    } else {
        border
    };
    k.c.border_focus = accent;
    k.c.border_active = accent;
    k.c.surface_base = bg;
    k.c.surface_weak = sidebar;
    k.c.surface_raised = panel;
    k.c.surface_raised_base = panel;
    k.c.surface_hover = hover;
    k.c.surface_raised_hover = hover;
    k.c.surface_float = panel;
    k.c.border_hover = accent;
    k.c.surface_disabled = sidebar;
    k.c.border_disabled = border;
    k.c.std_border = border;
    k.c.std_text = text;
    k.c.std_icon = muted;
    k.c.std_icon_strong = accent;
    k.c.text_weak = muted;
    k.c.success.icon = success;
    k.c.success.fg = success;
    k.c.success.bg = k.alpha(success, 0.12);
    k.c.danger.icon = danger;
    k.c.danger.fg = danger;
    k.c.danger.bg = k.alpha(danger, 0.12);
    k.radius.md = 6.0;
    k.radius.lg = 14.0;
    k.ty.body = TextStyle::new(15.0, 400.0).lh(26.0);
    k.ty.body_medium = TextStyle::new(15.0, 550.0).lh(26.0);
    k.ty.ui = TextStyle::new(12.0, 400.0).lh(20.0);
    k.ty.ui_medium = TextStyle::new(12.0, 500.0).lh(20.0);
    k.ty.ui_compact = TextStyle::new(12.0, 400.0).lh(16.0);
    k.ty.small = TextStyle::new(11.0, 400.0).lh(16.0);
    k.ty.small_medium = TextStyle::new(11.0, 550.0).lh(16.0);
    k.ty.text_xsmall_medium = k.ty.small_medium;
    k.ty.text_small = k.ty.ui;
    k.ty.text_small_medium = k.ty.ui_medium;
    k.ty.text_xsmall = k.ty.small;
    k.elevation.button_neutral.clear();
    k
}

pub fn markdown(k: &Kit) -> MarkdownStyle {
    MarkdownStyle::from_kit(k)
        .body_font(k.ty.body)
        .strong_weight(600.0)
        .heading_font(2, TextStyle::new(18.0, 600.0).lh(27.0))
        .heading_font(3, TextStyle::new(15.0, 600.0).lh(24.0))
        .code_block_font(TextStyle {
            size: 12.0,
            line_height: 26.0,
            ..k.ty.mono
        })
        .code_block_padding(16.0, 20.0)
        .code_block(k.c.bg_deep, k.c.border_base)
        .list(24.0, 4.0)
}
