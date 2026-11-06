//! Icons: the ones bundled with gpui-kit and the SVGs the client registers with `icons`
//! (PROTOCOL.md section 29). Every `icon` prop is drawn from `icon_path`.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, RwLock};

use gpui_kit::{AssetSource as _, SharedString};

/// Registered icons: name → (generation, SVG). Global: the asset source and the prop checks have
/// no session at hand.
static REGISTERED: LazyLock<RwLock<HashMap<String, (u64, String)>>> =
    LazyLock::new(Default::default);
static GENERATION: AtomicU64 = AtomicU64::new(0);

const REGISTERED_DIR: &str = "client-icons/";

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether `name` is one of the icons bundled with gpui-kit's default assets.
fn bundled(name: &str) -> bool {
    // shortcut: only the 101 default icons; registering `AllAssets` gives all 1,830 Lucide icons (+1 MB).
    valid_name(name)
        && matches!(
            gpui_kit::assets::Assets.load(&format!("icons/{name}.svg")),
            Ok(Some(_))
        )
}

/// Asset path of an icon. A registered one gets a new path each time it is registered, since
/// GPUI keeps a drawn SVG by its path.
pub fn icon_path(name: &str) -> String {
    match REGISTERED.read().unwrap().get(name) {
        Some((generation, _)) => format!("{REGISTERED_DIR}{generation}/{name}.svg"),
        None => format!("icons/{name}.svg"),
    }
}

/// Whether `name` is a bundled or a registered icon.
pub fn icon_exists(name: &str) -> bool {
    REGISTERED.read().unwrap().contains_key(name) || bundled(name)
}

/// Registers (or replaces) the client's icons. Every entry is checked first, so a rejected
/// message registers nothing.
pub fn register(icons: &BTreeMap<String, String>) -> Result<(), String> {
    for (name, svg) in icons {
        if !valid_name(name) {
            return Err(format!(
                "Icon name '{name}' must use lowercase letters, digits and '-'"
            ));
        }
        if bundled(name) {
            return Err(format!("'{name}' is a bundled icon and cannot be replaced"));
        }
        usvg::Tree::from_data(svg.as_bytes(), &usvg::Options::default())
            .map_err(|err| format!("Icon '{name}' is not a readable SVG: {err}"))?;
    }
    let mut registered = REGISTERED.write().unwrap();
    for (name, svg) in icons {
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed);
        registered.insert(name.clone(), (generation, svg.clone()));
    }
    Ok(())
}

/// gpui-kit's bundled icons plus the registered ones.
pub struct Assets;

impl gpui_kit::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        let Some(file) = path.strip_prefix(REGISTERED_DIR) else {
            return gpui_kit::assets::Assets.load(path);
        };
        let name = file
            .split_once('/')
            .and_then(|(_, file)| file.strip_suffix(".svg"));
        Ok(name.and_then(|name| {
            let registered = REGISTERED.read().unwrap();
            let (_, svg) = registered.get(name)?;
            Some(Cow::Owned(svg.clone().into_bytes()))
        }))
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M2 2h20v20z"/></svg>"#;

    fn icons(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
        entries
            .iter()
            .map(|(name, svg)| (name.to_string(), svg.to_string()))
            .collect()
    }

    // Names are unique per test: the registry is shared by tests running in parallel.
    #[test]
    fn a_registered_icon_is_served_and_moves_to_a_new_path_when_replaced() {
        assert!(!icon_exists("icons-test-logo"));
        register(&icons(&[("icons-test-logo", SVG)])).unwrap();
        assert!(icon_exists("icons-test-logo"));
        let first = icon_path("icons-test-logo");
        assert!(first.starts_with(REGISTERED_DIR));
        let served = Assets.load(&first).unwrap().unwrap();
        assert_eq!(&*served, SVG.as_bytes());

        register(&icons(&[("icons-test-logo", SVG)])).unwrap();
        assert_ne!(icon_path("icons-test-logo"), first);
        // Bundled icons keep their paths and still load.
        assert_eq!(icon_path("check"), "icons/check.svg");
        assert!(Assets.load("icons/check.svg").unwrap().is_some());
    }

    #[test]
    fn a_rejected_message_registers_nothing() {
        for bad in [
            icons(&[("icons-test-a", SVG), ("check", SVG)]),
            icons(&[("icons-test-a", SVG), ("icons-test-b", "not svg")]),
            icons(&[("icons-test-a", SVG), ("Icons Test", SVG)]),
        ] {
            assert!(register(&bad).is_err());
        }
        assert!(!icon_exists("icons-test-a"));
    }
}
