//! Декларативное описание раскладки главного окна (H1, шаг 1).
//!
//! Проблема: панели считали размеры из локального `available_width()`
//! в момент отрисовки — единой точки истины не было, блоки «плыли».
//! `LayoutSpec` фиксирует зоны поименно; `engine.rs` (шаг 2) будет
//! единственным местом, создающим панели. Значения пресетов —
//! из текущих хардкодов `app.rs`, поведение UI не меняется.

/// Раскладка главного окна: левая навигация, низ-статус, центр
/// и поля страницы. Все размеры — в поинтах egui (до `set_pixels_per_point`).
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSpec {
    /// Левая панель навигации.
    pub nav: SideZone,
    /// Нижняя строка статуса.
    pub status: Zone,
    /// Центральная область с вкладками.
    pub center: Zone,
    /// Симметричные поля страницы.
    pub page_margin: PageMargin,
}

/// Боковая зона фиксированной ширины.
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SideZone {
    /// Ширина по умолчанию (сейчас = min = max: нересайзбельно).
    pub default_w: f32,
    /// Минимальная ширина (для будущих ресайзбельных зон).
    pub min_w: f32,
    /// Максимальная ширина.
    pub max_w: f32,
    /// Можно ли тянуть границу мышью.
    pub resizable: bool,
    /// Свернуть зону, если ширина окна ниже (поинты). None = не сворачивать.
    pub collapse_below: Option<f32>,
}

/// Горизонтальная зона (верх/низ): фиксируется высотой.
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    /// Минимальная высота.
    pub min_h: f32,
    /// Максимальная высота.
    pub max_h: f32,
    /// Всегда видима (не уезжает при прокрутке).
    pub sticky: bool,
    /// Свернуть зону, если высота окна ниже (поинты). None = не сворачивать.
    pub collapse_below: Option<f32>,
}

/// Симметричные поля страницы: доля ширины окна с клампом.
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMargin {
    /// Доля ширины окна (0.05 = 5% с каждой стороны).
    pub percent: f32,
    /// Минимум в поинтах.
    pub min: f32,
    /// Максимум в поинтах.
    pub max: f32,
}

impl PageMargin {
    /// Поле для ширины окна `w` — та же формула, что `page_margin()` в app.rs.
    #[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
    pub fn for_width(self, w: f32) -> f32 {
        (w * self.percent).clamp(self.min, self.max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_spec_matches_current_constants() {
        // Сверка с хардкодами app.rs на момент H1-шаг-1:
        // - SidePanel::left("nav").exact_width(88.0)
        // - TopBottomPanel::bottom("status").exact_height(24.0)
        // - page_margin: percent 0.05, clamp 8..60
        let spec = crate::ui::layout::presets::default_layout();
        assert_eq!(spec.nav.default_w, 88.0);
        assert_eq!(spec.status.min_h, 24.0);
        assert_eq!(spec.page_margin.percent, 0.05);
        assert_eq!(spec.page_margin.min, 8.0);
        assert_eq!(spec.page_margin.max, 60.0);
    }

    #[test]
    fn page_margin_formula_matches_app_rs() {
        // for_width обязан повторять page_margin() из app.rs 1:1,
        // иначе пресет и прод разъедутся молча.
        let m = PageMargin { percent: 0.05, min: 8.0, max: 60.0 };
        assert_eq!(m.for_width(960.0), 48.0);
        assert_eq!(m.for_width(80.0), 8.0);
        assert_eq!(m.for_width(4000.0), 60.0);
    }
}
