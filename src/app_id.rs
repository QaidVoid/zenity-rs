//! Window identity used by the dialogs.
//!
//! `--app-id` sets the identity outright; `--class` and `--name` are the older
//! X11 resource options kept for compatibility. Any of them can be used on
//! either backend: on X11 they compose `WM_CLASS`, on Wayland they feed the
//! `xdg_toplevel` `app_id`. Values are taken as-is, without validation, so
//! short names such as `winetricks` work for taskbar matching.

use std::sync::RwLock;

#[cfg(feature = "x11")]
const DEFAULT_INSTANCE: &str = "zenity";
#[cfg(feature = "x11")]
const DEFAULT_CLASS: &str = "Zenity";
#[cfg(feature = "wayland")]
const DEFAULT_APP_ID: &str = "zenity";

#[derive(Default)]
struct Identity {
    app_id: Option<String>,
    /// `WM_CLASS` resource name (X11 `--name`).
    instance: Option<String>,
    /// `WM_CLASS` resource class (X11 `--class`).
    class: Option<String>,
}

static IDENTITY: RwLock<Identity> = RwLock::new(Identity {
    app_id: None,
    instance: None,
    class: None,
});

fn write_identity(f: impl FnOnce(&mut Identity)) {
    let mut identity = IDENTITY.write().unwrap_or_else(|e| e.into_inner());
    f(&mut identity);
}

/// X resource strings are C strings, so anything after an embedded NUL would
/// be ignored by GDK/Xlib. Truncate here to produce the same WM_CLASS.
fn resource_string(value: impl Into<String>) -> String {
    let mut value = value.into();
    if let Some(end) = value.find('\0') {
        value.truncate(end);
    }
    value
}

/// Overrides the process-wide application identifier.
///
/// The value is used as-is, without validation, so short names (e.g.
/// `winetricks`) work for window matching. Like the X11 setters, an embedded
/// NUL terminates it, which also keeps it safe for the Wayland string type.
/// It is read when each dialog window is created, so set it before showing one.
pub fn set_app_id(id: impl Into<String>) {
    let id = resource_string(id);
    write_identity(|identity| identity.app_id = Some(id));
}

/// Sets the X11 `WM_CLASS` resource class (the GTK `--class` option).
///
/// Takes precedence over the application ID for the class half of `WM_CLASS`.
/// It is read when each dialog window is created, so set it before showing one.
pub fn set_wm_class(class: impl Into<String>) {
    let class = resource_string(class);
    write_identity(|identity| identity.class = Some(class));
}

/// Sets the X11 `WM_CLASS` resource name (the GTK `--name` option).
///
/// Takes precedence over the application ID for the name half of `WM_CLASS`.
/// It is read when each dialog window is created, so set it before showing one.
pub fn set_wm_class_instance(instance: impl Into<String>) {
    let instance = resource_string(instance);
    write_identity(|identity| identity.instance = Some(instance));
}

/// Composes the X11 `WM_CLASS` property: two NUL-terminated strings, the
/// resource name followed by the resource class. `--name`/`--class` override
/// the corresponding half of the app ID.
#[cfg(feature = "x11")]
fn wm_class_from(identity: &Identity) -> Vec<u8> {
    let instance = identity
        .instance
        .as_deref()
        .or(identity.app_id.as_deref())
        .unwrap_or(DEFAULT_INSTANCE);
    let class = identity
        .class
        .as_deref()
        .or(identity.app_id.as_deref())
        .unwrap_or(DEFAULT_CLASS);

    let mut value = Vec::with_capacity(instance.len() + class.len() + 2);
    value.extend_from_slice(instance.as_bytes());
    value.push(0);
    value.extend_from_slice(class.as_bytes());
    value.push(0);
    value
}

/// The `WM_CLASS` property to put on X11 windows.
#[cfg(feature = "x11")]
pub(crate) fn wm_class() -> Vec<u8> {
    let identity = IDENTITY.read().unwrap_or_else(|e| e.into_inner());
    wm_class_from(&identity)
}

/// Resolves the Wayland `app_id`: `--app-id` wins, otherwise `--name` then
/// `--class`, the same precedence GTK3 gives them on Wayland.
#[cfg(feature = "wayland")]
fn app_id_from(identity: &Identity) -> String {
    identity
        .app_id
        .as_deref()
        .or(identity.instance.as_deref())
        .or(identity.class.as_deref())
        .unwrap_or(DEFAULT_APP_ID)
        .to_string()
}

/// The `app_id` to put on Wayland toplevels.
#[cfg(feature = "wayland")]
pub(crate) fn wayland_app_id() -> String {
    let identity = IDENTITY.read().unwrap_or_else(|e| e.into_inner());
    app_id_from(&identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_id_is_taken_verbatim() {
        let identity = Identity {
            app_id: Some("winetricks".into()),
            ..Identity::default()
        };
        #[cfg(feature = "x11")]
        assert_eq!(wm_class_from(&identity), b"winetricks\0winetricks\0");
        #[cfg(feature = "wayland")]
        assert_eq!(app_id_from(&identity), "winetricks");
    }

    #[test]
    fn resource_strings_stop_at_nul() {
        assert_eq!(resource_string("My\0Trailing"), "My");
    }

    #[cfg(feature = "x11")]
    #[test]
    fn wm_class_defaults_match_gtk() {
        assert_eq!(wm_class_from(&Identity::default()), b"zenity\0Zenity\0");
    }

    #[cfg(feature = "x11")]
    #[test]
    fn wm_class_uses_app_id_for_both_halves() {
        let identity = Identity {
            app_id: Some("com.example.MyApp".into()),
            ..Identity::default()
        };
        assert_eq!(
            wm_class_from(&identity),
            b"com.example.MyApp\0com.example.MyApp\0"
        );
    }

    #[cfg(feature = "x11")]
    #[test]
    fn name_and_class_override_app_id_per_half() {
        let identity = Identity {
            app_id: Some("com.example.MyApp".into()),
            instance: Some("CustomName".into()),
            class: Some("CustomClass".into()),
        };
        assert_eq!(wm_class_from(&identity), b"CustomName\0CustomClass\0");

        let only_name = Identity {
            app_id: None,
            instance: Some("MyName".into()),
            ..Identity::default()
        };
        assert_eq!(wm_class_from(&only_name), b"MyName\0Zenity\0");

        let only_class = Identity {
            app_id: None,
            class: Some("MyClass".into()),
            ..Identity::default()
        };
        assert_eq!(wm_class_from(&only_class), b"zenity\0MyClass\0");
    }

    #[cfg(feature = "wayland")]
    #[test]
    fn wayland_app_id_falls_back_to_name_then_class() {
        assert_eq!(app_id_from(&Identity::default()), "zenity");

        let by_class = Identity {
            class: Some("MyClass".into()),
            ..Identity::default()
        };
        assert_eq!(app_id_from(&by_class), "MyClass");

        let by_name = Identity {
            instance: Some("MyName".into()),
            class: Some("MyClass".into()),
            ..Identity::default()
        };
        assert_eq!(app_id_from(&by_name), "MyName");

        let by_app_id = Identity {
            app_id: Some("com.example.MyApp".into()),
            instance: Some("MyName".into()),
            ..Identity::default()
        };
        assert_eq!(app_id_from(&by_app_id), "com.example.MyApp");
    }
}
