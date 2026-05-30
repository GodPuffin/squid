use super::{ColorScheme, Theme};

#[test]
fn schemes_produce_distinct_accent_colors() {
    let dark = Theme::from_scheme(ColorScheme::Dark);
    let light = Theme::from_scheme(ColorScheme::Light);
    let dracula = Theme::from_scheme(ColorScheme::Dracula);

    assert_ne!(dark.accent, light.accent);
    assert_ne!(dark.background, light.background);
    assert_eq!(dracula.scheme, ColorScheme::Dracula);
}

#[test]
fn color_scheme_cycles_through_all_variants() {
    assert_eq!(ColorScheme::Dark.cycle(), ColorScheme::Light);
    assert_eq!(ColorScheme::Dracula.cycle(), ColorScheme::Dark);
}
