//! Пресеты раскладки (H1, шаг 1).
//!
//! Значения — слепок текущих хардкодов `app.rs`. UI не меняется:
//! engine (шаг 2) прочитает этот же пресет и построит те же панели.

use super::spec::{LayoutSpec, PageMargin, SideZone, Zone};

/// Раскладка как сейчас: навигация 88pt фикс, статус 24pt фикс,
/// центр flex, поля 5% с клампом 8..60.
#[allow(dead_code)] // TODO(H1-step-2): потребитель — engine::build_window
pub fn default_layout() -> LayoutSpec {
    LayoutSpec {
        nav: SideZone {
            default_w: 88.0,
            min_w: 88.0,
            max_w: 88.0,
            resizable: false,
            collapse_below: None,
        },
        status: Zone {
            min_h: 24.0,
            max_h: 24.0,
            sticky: true,
            collapse_below: None,
        },
        center: Zone {
            min_h: 0.0,
            max_h: f32::INFINITY,
            sticky: false,
            collapse_below: None,
        },
        page_margin: PageMargin {
            percent: 0.05,
            min: 8.0,
            max: 60.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_match_app_rs_hardcodes() {
        // Эти значения обязаны совпадать с app.rs:
        // - SidePanel::left("nav").exact_width(88.0)
        // - TopBottomPanel::bottom("status").exact_height(24.0)
        // - page_margin: percent 0.05, clamp 8..60
        let spec = default_layout();
        assert_eq!(spec.nav.default_w, 88.0, "nav width");
        assert_eq!(spec.status.min_h, 24.0, "status height");
        assert_eq!(spec.status.max_h, 24.0, "status height");
        assert!(!spec.nav.resizable, "nav resizable");
        assert_eq!(spec.page_margin.percent, 0.05, "page margin %");
        assert_eq!(spec.page_margin.min, 8.0, "page margin min");
        assert_eq!(spec.page_margin.max, 60.0, "page margin max");
    }

    #[test]
    fn spec_is_copy() {
        // Spec копируется, а не клонируется в heap: engine читает его
        // каждый кадр, аллокаций быть не должно.
        fn assert_copy<T: Copy>() {}
        assert_copy::<LayoutSpec>();
        let a = default_layout();
        let b = a;
        assert_eq!(a, b);
    }
}
