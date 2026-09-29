//! Токены оформления.
//!
//! Этап 1.1: только структуры, без привязки к реальным цветам программы.
//! Наполнение (`default.rs` реальными цветами из аудита) — коммит 1.3.

use egui::Color32;

/// Отступы и фиксированные размеры каркаса (в поинтах).
#[allow(dead_code)] // TODO(phase-1.2): поля читаются при замене хардкодов в app.rs
#[derive(Debug, Clone)]
pub struct Spacing {
    /// Крошечный зазор (4).
    pub xs: f32,
    /// Малый зазор (8).
    pub sm: f32,
    /// Средний зазор (12).
    pub md: f32,
    /// Крупный зазор (16).
    pub lg: f32,
    /// Ширина левой навигации (88 — как сейчас в `SidePanel`).
    pub nav_w: f32,
    /// Высота строки статуса (24 — как сейчас в `TopBottomPanel`).
    pub status_h: f32,
    /// Высота строки списка.
    pub row_h: f32,
}

/// Радиусы скругления.
#[allow(dead_code)] // TODO(phase-1.2)
#[derive(Debug, Clone)]
pub struct Radii {
    /// Виджеты (кнопки, поля).
    pub widget: f32,
    /// Панели и карточки.
    pub panel: f32,
}

/// Размеры шрифта (в поинтах).
#[allow(dead_code)] // TODO(phase-1.2)
#[derive(Debug, Clone)]
pub struct Typography {
    /// Основной текст.
    pub body: f32,
    /// Подписи и мелкий текст.
    pub caption: f32,
    /// Заголовки разделов.
    pub heading: f32,
}

/// Палитра скина.
#[allow(dead_code)] // TODO(phase-1.2/1.3): поля читаются при замене цветов и наполнении default
#[derive(Debug, Clone)]
pub struct Palette {
    /// Фон панелей.
    pub surface: Color32,
    /// Альтернативный фон (карточки, чётные строки).
    pub surface_alt: Color32,
    /// Акцент (выбранная вкладка, ссылки).
    pub accent: Color32,
    /// Основной текст.
    pub text: Color32,
    /// Приглушённый текст.
    pub text_muted: Color32,
    /// Рамки и разделители.
    pub border: Color32,
    /// Успех.
    pub ok: Color32,
    /// Предупреждение.
    pub warn: Color32,
    /// Ошибка.
    pub err: Color32,
}

/// Полный набор токенов скина.
#[allow(dead_code)] // TODO(phase-1.2)
#[derive(Debug, Clone)]
pub struct Tokens {
    pub spacing: Spacing,
    pub radii: Radii,
    pub typography: Typography,
    pub palette: Palette,
}
