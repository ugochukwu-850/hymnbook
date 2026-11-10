//! Faith Tabernacle Hymn Book: a desktop hymn book with playable tunes.

mod app;
mod audio;
mod hymn;
mod theme;
mod tunes;
mod ui;

use std::borrow::Cow;

use gpui_kit::*;

/// Fonts are bundled so the app looks identical on every operating system.
const FONTS: [&[u8]; 6] = [
    include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSans-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSerif-Bold.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Regular.ttf"),
    include_bytes!("../assets/fonts/NotoSansMono-Bold.ttf"),
];

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        if let Err(e) = cx
            .text_system()
            .add_fonts(FONTS.iter().map(|f| Cow::Borrowed(*f)).collect())
        {
            eprintln!("could not load bundled fonts: {e}");
        }
        component::Theme::change(component::ThemeMode::Dark, None, cx);

        let bounds = Bounds::centered(None, size(px(1280.0), px(820.0)), cx);
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                is_minimizable: true,
                is_movable: true,
                is_resizable: true,
                // We draw our own titlebar (GNOME/Wayland has no server-side decorations).
                window_decorations: Some(WindowDecorations::Client),
                titlebar: Some(TitlebarOptions {
                    title: Some("Hymn Book".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                ..WindowOptions::default()
            },
            cx,
            |window, cx| cx.new(|cx| app::Audible::new(window, cx)),
        )
        .expect("Failed to open window");
    });
}
