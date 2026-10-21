//! The platform windows: opens, updates and closes them to match the tree's window specs, and applies the app-wide theme.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use crate::Session;
use crate::protocol::{CustomThemes, HostMessage, WindowId as ClientWindowId, WindowTheme};
use crate::render::RootView;
use crate::transport;
use crate::tree::{ThemeSpec, WindowSpec};

const DEFAULT_SIZE: (f32, f32) = (800., 600.);

/// A window the host opened, with the spec last applied to it.
struct OpenWindow {
    handle: AnyWindowHandle,
    applied: WindowSpec,
}

/// Open windows and the theme last applied, shared by `sync_windows` and `window_closed`.
#[derive(Default)]
pub struct Windows {
    open: BTreeMap<ClientWindowId, OpenWindow>,
    theme: ThemeSpec,
}

impl Windows {
    /// The platform window of an open client window.
    pub fn handle(&self, id: ClientWindowId) -> Option<AnyWindowHandle> {
        self.open.get(&id).map(|window| window.handle)
    }
}

/// Applies a commit to the platform windows: the theme, then opens, updates and closes windows.
pub fn sync_windows(session: &Entity<Session>, windows: &RefCell<Windows>, cx: &mut App) {
    let (specs, theme) = {
        let tree = &session.read(cx).tree;
        (tree.windows().clone(), tree.theme().clone())
    };
    // Applied before opening windows too, so none flashes the wrong theme.
    let applied_theme = std::mem::replace(&mut windows.borrow_mut().theme, theme.clone());
    apply_theme(&theme, &applied_theme, cx);

    // Closed by the client: dropped from `open` first, so `window_closed` does not report them.
    let closed: Vec<OpenWindow> = {
        let mut windows = windows.borrow_mut();
        let gone: Vec<ClientWindowId> = windows
            .open
            .keys()
            .filter(|id| !specs.contains_key(id))
            .copied()
            .collect();
        gone.iter()
            .filter_map(|id| windows.open.remove(id))
            .collect()
    };
    for window in closed {
        let _ = window
            .handle
            .update(cx, |_, window, _| window.remove_window());
    }

    // `minimized` is an action (PROTOCOL.md section 3): applied after opening or updating the
    // window, so `{"minimized":true}` works in the same commit that opens it.
    let requests = session.update(cx, |session, _| session.tree.take_window_requests());

    for (id, spec) in specs {
        let opened = windows
            .borrow()
            .open
            .get(&id)
            .map(|window| (window.handle, window.applied.clone()));
        match opened {
            None if spec.root.is_some() => {
                if let Some(handle) = open_window(session, id, &spec, cx) {
                    windows.borrow_mut().open.insert(
                        id,
                        OpenWindow {
                            handle,
                            applied: spec,
                        },
                    );
                }
            }
            None => {}
            Some((handle, applied)) if applied != spec => {
                update_window(handle, &applied, &spec, cx);
                if let Some(window) = windows.borrow_mut().open.get_mut(&id) {
                    window.applied = spec;
                }
            }
            Some(_) => {}
        }
    }

    for (id, minimized) in requests {
        let Some(handle) = windows.borrow().open.get(&id).map(|window| window.handle) else {
            log::warn!("'minimized' for window {id}, which is not open; ignored");
            continue;
        };
        let result = handle.update(cx, |_, window, _| {
            if minimized {
                window.minimize_window();
            } else {
                // Restores a minimized window and brings it to the front.
                window.activate_window();
            }
        });
        if let Err(err) = result {
            log::warn!("failed to apply 'minimized' to window {id}: {err:#}");
        }
    }
}

/// The user closed a platform window: report it, forget it, and quit after the last one unless there is a tray.
pub fn window_closed(
    session: &Entity<Session>,
    windows: &RefCell<Windows>,
    closed: WindowId,
    cx: &mut App,
) {
    let (id, last) = {
        let mut windows = windows.borrow_mut();
        // Not found: the client closed it with `closeWindow`.
        let Some(id) = windows
            .open
            .iter()
            .find(|(_, window)| window.handle.window_id() == closed)
            .map(|(&id, _)| id)
        else {
            return;
        };
        windows.open.remove(&id);
        (id, windows.open.is_empty())
    };
    let tray = session.update(cx, |session, _| {
        session.tree.close_window(id);
        session.tree.tray().is_some()
    });
    transport::send(&HostMessage::WindowClosed { id });
    if last && !tray {
        cx.quit();
    }
}

fn size_of(spec: &WindowSpec) -> Size<Pixels> {
    size(
        px(spec.width.unwrap_or(DEFAULT_SIZE.0)),
        px(spec.height.unwrap_or(DEFAULT_SIZE.1)),
    )
}

/// Where the window opens: at `x`/`y` when either was given, centered otherwise.
fn bounds_of(spec: &WindowSpec, cx: &mut App) -> WindowBounds {
    let size = size_of(spec);
    match (spec.x, spec.y) {
        (None, None) => WindowBounds::centered(size, cx),
        (x, y) => WindowBounds::Windowed(Bounds {
            origin: point(px(x.unwrap_or(0.0)), px(y.unwrap_or(0.0))),
            size,
        }),
    }
}

/// `minWidth`/`minHeight`; a missing one leaves that axis unconstrained.
fn min_size_of(spec: &WindowSpec) -> Option<Size<Pixels>> {
    match (spec.min_width, spec.min_height) {
        (None, None) => None,
        (width, height) => Some(size(px(width.unwrap_or(0.0)), px(height.unwrap_or(0.0)))),
    }
}

fn open_window(
    session: &Entity<Session>,
    id: ClientWindowId,
    spec: &WindowSpec,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let options = WindowOptions {
        window_bounds: Some(bounds_of(spec, cx)),
        window_min_size: min_size_of(spec),
        titlebar: Some(TitlebarOptions {
            title: Some(spec.title.clone().unwrap_or_default().into()),
            // Hides the system title bar; the window keeps its resize borders, shadow and taskbar title.
            appears_transparent: spec.frameless.unwrap_or(false),
            ..Default::default()
        }),
        ..Default::default()
    };
    let session = session.clone();
    match gpui_kit::open_window(options, cx, |window, cx| {
        cx.new(|cx| RootView::new(session, id, window, cx))
    }) {
        Ok((handle, _)) => Some(handle),
        Err(err) => {
            log::error!("failed to open window {id}: {err:#}");
            None
        }
    }
}

/// Only touches what changed, so a user resize is not undone by a title update.
fn update_window(handle: AnyWindowHandle, applied: &WindowSpec, spec: &WindowSpec, cx: &mut App) {
    if spec.frameless.unwrap_or(false) != applied.frameless.unwrap_or(false) {
        log::warn!("'frameless' only applies when the window opens; ignored");
    }
    if (spec.x, spec.y) != (applied.x, applied.y) {
        log::warn!("'x'/'y' only apply when the window opens; ignored");
    }
    if (spec.min_width, spec.min_height) != (applied.min_width, applied.min_height) {
        log::warn!("'minWidth'/'minHeight' only apply when the window opens; ignored");
    }
    let result = handle.update(cx, |_, window, _| {
        if spec.title != applied.title {
            window.set_window_title(spec.title.as_deref().unwrap_or(""));
        }
        if (spec.width, spec.height) != (applied.width, applied.height) {
            window.resize(size_of(spec));
        }
    });
    if let Err(err) = result {
        log::warn!("failed to update window: {err:#}");
    }
}

fn apply_theme(spec: &ThemeSpec, applied: &ThemeSpec, cx: &mut App) {
    let mode_changed = spec.theme.is_some() && spec.theme != applied.theme;
    let set_changed = spec.theme_set.is_some() && spec.theme_set != applied.theme_set;
    if let Some(CustomThemes(set)) = spec.theme_set.as_ref().filter(|_| set_changed) {
        // A mode the set does not define keeps the default theme.
        let pick = |mode: ThemeMode, default: &Rc<ThemeConfig>| {
            set.themes
                .iter()
                .find(|config| config.mode == mode)
                .map_or_else(|| default.clone(), |config| Rc::new(config.clone()))
        };
        let registry = ThemeRegistry::global(cx);
        let light = pick(ThemeMode::Light, registry.default_light_theme());
        let dark = pick(ThemeMode::Dark, registry.default_dark_theme());
        let theme = Theme::global_mut(cx);
        theme.light_theme = light;
        theme.dark_theme = dark;
    }
    if mode_changed || set_changed {
        let mode = match spec.theme.unwrap_or(WindowTheme::Light) {
            WindowTheme::Light => ThemeMode::Light,
            WindowTheme::Dark => ThemeMode::Dark,
        };
        // A theme file only sets these when it defines them; reset them so none leaks from the previous theme.
        let defaults = Theme::default();
        let theme = Theme::global_mut(cx);
        theme.font_size = defaults.font_size;
        theme.font_family = defaults.font_family;
        theme.mono_font_size = defaults.mono_font_size;
        theme.mono_font_family = defaults.mono_font_family;
        theme.radius = defaults.radius;
        theme.radius_lg = defaults.radius_lg;
        theme.shadow = defaults.shadow;
        // Also reloads the mode's theme, so a new set applies even when the mode is unchanged; every window repaints.
        Theme::change(mode, None, cx);
    }
}
