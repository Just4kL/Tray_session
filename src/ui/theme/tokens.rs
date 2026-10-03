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

#[derive(Clone, Copy)]
pub struct Strokes {
    pub hairline: f32, // 1.0
    pub cursor:   f32, // 2.0
}

#[allow(dead_code)] // TODO(phase-1.2.a.2)
#[derive(Clone, Copy)]
pub struct Typography { pub body: f32, pub caption: f32, pub heading: f32 }

/// Сетка раскладки (H1): базовый шаг и «блок».
///
/// Размеры зон и поля страницы задаются в единицах сетки, а не в
/// «магических» поинтах: `u(11.0)` читается как «одиннадцать шагов».
/// Держится в токенах темы, чтобы спека раскладки и скин считали
/// сетку из одного места.
#[derive(Clone, Copy)]
pub struct Grid {
    /// Базовый шаг сетки, поинты. К нему выравниваются размеры зон
    /// и поля страницы.
    pub unit: f32,
    /// Крупный шаг — «блок» из 8 единиц, поинты.
    pub block: f32,
}

impl Grid {
    /// `n` единиц сетки: `u(11.0)` = 88pt при `unit = 8.0`.
    pub fn u(self, n: f32) -> f32 { self.unit * n }
    /// `n` блоков сетки: `b(1.0)` = 64pt при `block = 64.0`.
    #[allow(dead_code)] // TODO(phase-2): потребитель — крупные блоки раскладки
    pub fn b(self, n: f32) -> f32 { self.block * n }
}

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

#[derive(Clone, Copy)]
pub struct InteractiveState { pub bg: Color32, pub weak: Color32 }

#[derive(Clone, Copy)]
pub struct InteractivePalette {
    pub inactive: InteractiveState,
    pub hovered:  InteractiveState,
    pub active:   InteractiveState,
}

#[derive(Clone, Copy)]
pub struct AccentPalette {
    pub primary:   Color32,
    pub selection: Color32,
    pub bright:    Color32,
    /// Приглушённый акцент для рамок оверлеев. Производный цвет, заведён
    /// именованным токеном вместо константы, чтобы не плодить «почти акценты».
    pub dim: Color32,
}

#[derive(Clone, Copy)]
pub struct TextPalette {
    pub primary:   Color32,
    pub muted:     Color32,
    pub on_accent: Color32,
    /// Заголовки страниц 21pt. Отдельный оттенок, не сводится к on_accent
    /// (тот ярче — чистый белый).
    pub heading: Color32,
}

#[derive(Clone, Copy)]
pub struct BorderPalette {
    #[allow(dead_code)] // TODO(фаза 2): потребитель default пока не переведён на токены
    pub default:     Color32,
    pub interactive: Color32,
    /// Рамки карточек. Темнее интерактивной — карточка не должна
    /// спорить с кнопками.
    pub subtle: Color32,
}

#[derive(Clone, Copy)]
pub struct SemanticPalette {
    pub ok:        Color32,
    pub warn:      Color32,
    #[allow(dead_code)] // Update notice now uses the blue accent (M1).
    pub warn_soft: Color32,
    #[allow(dead_code)] // TODO(фаза 2): потребитель err пока не переведён на токены
    pub err:       Color32,
    /// Жёлтый системный сигнал («идёт проверка»). Токена yellow нет
    /// осознанно: это не часть темы, а состояние процесса.
    pub attention: Color32,
    /// Зелёный системный сигнал («идёт запись»). Пара к `attention`.
    pub recording: Color32,
}

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
    pub grid:       Grid,
    pub palette:    Palette,
}
