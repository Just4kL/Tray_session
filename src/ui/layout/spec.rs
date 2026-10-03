//! Декларативное описание раскладки главного окна (H1, шаг 1).
//!
//! Проблема: панели считали размеры из локального `available_width()`
//! в момент отрисовки — единой точки истины не было, блоки «плыли».
//! `LayoutSpec` фиксирует зоны поименно; `engine.rs` (шаг 2) будет
//! единственным местом, создающим панели. Значения пресетов —
//! из текущих хардкодов `app.rs`, поведение UI не меняется.

use crate::ui::theme::default::DefaultSkin;
use crate::ui::theme::tokens::Grid;
use crate::ui::theme::Skin;

/// Сетка раскладки (H1): шаг берём у скина, а не хардкодим 8.0 второй раз.
/// Спека и тема обязаны считать сетку одинаково — это стережёт
/// `grid_matches_the_skin`.
pub fn grid() -> Grid {
    DefaultSkin::default().tokens().grid
}

/// Раскладка главного окна: левая навигация, низ-статус, центр
/// и поля страницы. Все размеры — в поинтах egui (до `set_pixels_per_point`).
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSpec {
    /// Пользовательская верхняя панель окна.
    pub titlebar: Zone,
    /// Левая панель навигации.
    pub nav: SideZone,
    /// Правая контекстная панель настройки.
    pub configurator: SideZone,
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
    /// Поле для ширины окна `w`: доля ширины с клампом, затем выравнивание
    /// по сетке темы. Это ЕДИНСТВЕННАЯ реализация формулы — `page_margin()`
    /// в app.rs делегирует сюда, поэтому пресет и прод не разъедутся.
    #[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
    pub fn for_width(self, w: f32) -> f32 {
        let m = (w * self.percent).clamp(self.min, self.max);
        // H1: поля обязаны лежать на сетке — иначе блоки «плывут» на
        // пол-шага. Верхний кламп 60pt сдвигается к ближайшему шагу — 64pt.
        let u = grid().unit;
        (m / u).round() * u
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
        // for_width — единственная реализация формулы; page_margin()
        // в app.rs делегирует сюда, поэтому разъехаться они не могут.
        let m = PageMargin { percent: 0.05, min: 8.0, max: 60.0 };
        assert_eq!(m.for_width(960.0), 48.0);
        assert_eq!(m.for_width(80.0), 8.0);
        // Верхний кламп 60pt выравнивается по сетке 8pt → 64pt.
        assert_eq!(m.for_width(4000.0), 64.0);
    }

    #[test]
    fn grid_matches_the_skin() {
        // Сетка раскладки — из токенов темы; спека не заводит свои 8.0/64.0.
        let g = grid();
        assert_eq!(g.unit, 8.0);
        assert_eq!(g.block, 64.0);
        assert_eq!(g.u(11.0), 88.0);
        assert_eq!(g.b(1.0), 64.0);
    }

    #[test]
    fn nav_and_status_are_whole_grid_units() {
        // H1: nav = 11u = 88pt, status = 3u = 24pt.
        let g = grid();
        let spec = crate::ui::layout::presets::default_layout();
        assert_eq!(spec.nav.default_w / g.unit, 11.0, "nav = 11u");
        assert_eq!(spec.status.min_h / g.unit, 3.0, "status = 3u");
    }

    #[test]
    fn all_spec_point_values_lie_on_the_grid() {
        // Каждое ПОИНТОВОЕ значение спеки кратно шагу сетки. В проверку
        // не входят: `page_margin.percent` (доля ширины, не поинты),
        // `center.max_h` (INFINITY) и `page_margin.min/max` — это границы
        // клампа, а не размеры; на сетку ложится их результат, и это
        // отдельно проверяет `page_margin_result_lies_on_the_grid`.
        let g = grid();
        let spec = crate::ui::layout::presets::default_layout();
        let points = [
            ("titlebar.min_h", spec.titlebar.min_h),
            ("nav.default_w", spec.nav.default_w),
            ("nav.min_w", spec.nav.min_w),
            ("nav.max_w", spec.nav.max_w),
            ("configurator.default_w", spec.configurator.default_w),
            ("configurator.min_w", spec.configurator.min_w),
            ("configurator.max_w", spec.configurator.max_w),
            ("status.min_h", spec.status.min_h),
            ("status.max_h", spec.status.max_h),
            ("center.min_h", spec.center.min_h),
            ("page_margin.min", spec.page_margin.min),
        ];
        for (name, v) in points {
            assert_eq!(v % g.unit, 0.0, "{name} = {v} не кратно {}pt", g.unit);
        }
    }

    #[test]
    fn page_margin_result_lies_on_the_grid() {
        // Снап обязан работать на всём диапазоне ширин окна, включая
        // участок, где кламп упирается в верхнюю границу 60pt.
        let g = grid();
        let spec = crate::ui::layout::presets::default_layout();
        for w in [320.0, 720.0, 960.0, 1200.0, 1920.0, 3840.0, 5760.0] {
            let m = spec.page_margin.for_width(w);
            assert_eq!(m % g.unit, 0.0, "поле {m} при ширине {w} не кратно {}pt", g.unit);
            assert!(m >= spec.page_margin.min, "поле {m} меньше минимума");
        }
    }
}
