// Minimal i18n. Keys are namespaced by section. Only en-us shipped for now;
// the structure mirrors ndxzstudio/lang/{locale}/ so additional locales can be
// added later as one Rust module per locale.

#![allow(dead_code)] // Scaffolding for templates that adopt i18n incrementally.

use std::collections::HashMap;
use std::sync::OnceLock;

pub struct Locale {
    pub code: &'static str,
    pub strings: HashMap<&'static str, &'static str>,
}

static EN_US: OnceLock<Locale> = OnceLock::new();

pub fn lookup(_lang: &str, key: &str) -> String {
    // Only en-us shipped for now. Future: dispatch on `lang`.
    let locale = en_us();
    locale
        .strings
        .get(key)
        .copied()
        .map_or_else(|| key.to_string(), str::to_string)
}

fn en_us() -> &'static Locale {
    EN_US.get_or_init(|| {
        let mut s = HashMap::new();
        s.insert("admin.dashboard.title", "Dashboard");
        s.insert("admin.exhibits.title", "Exhibits");
        s.insert("admin.exhibits.new", "New exhibit");
        s.insert("admin.exhibits.edit", "Edit exhibit");
        s.insert("admin.exhibits.delete", "Delete");
        s.insert("admin.sections.title", "Sections");
        s.insert("admin.tags.title", "Tags");
        s.insert("admin.users.title", "Users");
        s.insert("admin.settings.title", "Settings");
        s.insert("admin.media.title", "Media");
        s.insert("admin.media.upload", "Upload");
        s.insert("admin.login.title", "Log in");
        s.insert("admin.login.submit", "Log in");
        s.insert("admin.logout.submit", "Log out");
        s.insert("admin.action.save", "Save");
        s.insert("admin.action.cancel", "Cancel");
        s.insert("admin.action.delete", "Delete");
        s.insert("public.password.title", "Password required");
        s.insert(
            "public.password.prompt",
            "This exhibit is password-protected.",
        );
        s.insert("public.password.submit", "Enter");
        s.insert("public.password.error", "Incorrect password.");
        Locale {
            code: "en-us",
            strings: s,
        }
    })
}
