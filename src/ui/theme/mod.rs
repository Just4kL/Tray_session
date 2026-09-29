//! Тема оформления.
//!
//! Этап 1.1: структуры `Skin` + `ThemeManager`, скины-заглушки
//! (`default.rs`, `dark.rs`). `layout_spec()` из трейта убран намеренно —
//! вернётся в фазе 2, тащить фиктивный `LayoutSpec` в 1.1 не нужно.

pub mod dark;
pub mod default;
pub mod tokens;

use egui::Context;
use std::sync::Arc;
use tokens::Tokens;

/// Скин: именованный набор токенов плюс маппинг в стиль egui.
#[allow(dead_code)] // TODO(phase-1.2): методы читаются из update() при замене хардкодов
pub trait Skin: Send + Sync {
    /// Id скина (`"default"`, `"dark"`). По нему `update()` решает,
    /// вызывать ли `ctx.set_style`.
    fn id(&self) -> &'static str;
    /// Токены скина.
    fn tokens(&self) -> &Tokens;
    /// Маппинг токенов в `egui::Style`. В 1.1 возвращает стиль по
    /// умолчанию и нигде не вызывается — наполнение в 1.3.
    fn egui_style(&self) -> egui::Style;
}

/// Владелец текущей темы.
#[allow(dead_code)] // TODO(phase-1.2/1.4): current() читается из update(), set() — из переключателя
pub struct ThemeManager {
    current: Arc<dyn Skin>,
}

#[allow(dead_code)] // TODO(phase-1.2/1.4): current()/set() задействуются следующими коммитами
impl ThemeManager {
    /// Создать менеджер с начальным скином.
    pub fn new(initial: Arc<dyn Skin>) -> Self {
        Self { current: initial }
    }

    /// Текущий скин. Вызывается с фазы 1.2 (чтение токенов в `update()`).
    pub fn current(&self) -> &Arc<dyn Skin> {
        &self.current
    }

    /// Сменить скин и сразу применить стиль. Вызывается с фазы 1.4
    /// (переключатель в «О программе»).
    pub fn set(&mut self, s: Arc<dyn Skin>, ctx: &Context) {
        self.current = s;
        ctx.set_style(self.current.egui_style());
    }
}
