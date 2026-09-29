//! Токены оформления.
//!
//! Этап 1.2.a.1: полный набор структур, выведенный из аудита
//! `steam_visuals()`. Значения — в `default.rs` (реальные цвета) и
//! `dark.rs` (копия до фазы 1.4). Чтение токенов — с 1.2.a.2
//! (`steam_visuals()` -> `egui_style()`), пока всё под `allow`.

use egui::Color32;

#[allow(dead_code)] // TODO(phase-1.2.a.2): steam_visuals() -> egui_style()
#[derive(Clone, Copy)]
pub struct Spacing {
    pub xs: f32, pub sm: f32, pub md: f32, pub lg: f32,
    pub nav_w: f32, pub status_h: f32, pub row_h: f32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct Radii {
    pub widget: f32, // 6.0
    pub panel:  f32, // 8.0
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct Strokes {
    pub hairline: f32, // 1.0
    pub cursor:   f32, // 2.0
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct Typography { pub body: f32, pub caption: f32, pub heading: f32 }

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct BgPalette {
    pub surface:        Color32,
    pub window:         Color32,
    pub extreme:        Color32,
    pub faint:          Color32,
    pub sunken:         Color32,
    pub noninteractive: Color32,
    pub raised:         Color32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct InteractiveState { pub bg: Color32, pub weak: Color32 }

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct InteractivePalette {
    pub inactive: InteractiveState,
    pub hovered:  InteractiveState,
    pub active:   InteractiveState,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct AccentPalette {
    pub primary:   Color32,
    pub selection: Color32,
    pub bright:    Color32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct TextPalette {
    pub primary:   Color32,
    pub muted:     Color32,
    pub on_accent: Color32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct BorderPalette {
    pub default:     Color32,
    pub interactive: Color32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct SemanticPalette {
    pub ok:        Color32,
    pub warn:      Color32,
    pub warn_soft: Color32,
    pub err:       Color32,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg:          BgPalette,
    pub interactive: InteractivePalette,
    pub accent:      AccentPalette,
    pub text:        TextPalette,
    pub border:      BorderPalette,
    pub semantic:    SemanticPalette,
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
pub struct Tokens {
    pub spacing:    Spacing,
    pub radii:      Radii,
    pub strokes:    Strokes,
    pub typography: Typography,
    pub palette:    Palette,
}
