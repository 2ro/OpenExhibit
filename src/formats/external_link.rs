// External-link "exhibit": doesn't render its own page — it's a nav-only
// entry that bounces visitors to the configured URL.
//
// Two things distinguish it from a layout format:
//   * `nav_href` returns the exhibit's `link` column instead of its
//     internal URL, so the nav anchor points outward.
//   * `intercept` returns a 302 so directly visiting the exhibit's slug
//     also redirects.
//
// `link` is validated at admin write time in `routes/admin/exhibits.rs`
// (`sanitize_external_link`) — `javascript:` / `data:` etc. never reach
// the DB, so the scheme here is always one of the allowlist.

use actix_web::HttpResponse;
use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, FormatCapabilities, MediaView, NavHref};

#[derive(Template)]
#[template(path = "public/formats/external_link.html")]
struct Page {
    base: BaseFields,
    link: String,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "external_link"
    }
    fn display_name(&self) -> &'static str {
        "External link"
    }
    fn description(&self) -> &'static str {
        "Nav-only entry — clicking sends the visitor to an external URL. \
         Useful for shop / Instagram / CV PDF links."
    }
    fn capabilities(&self) -> FormatCapabilities {
        FormatCapabilities {
            uses_media: false,
            uses_content: false,
            uses_external_link: true,
            uses_password: false,
            uses_thumbs_size: false,
            requires_url_slug: false,
        }
    }
    fn nav_href(&self, exhibit: &Exhibit) -> NavHref {
        // Empty or scheme-bad `link` falls back to the internal URL —
        // the visit-side `intercept` returns None in that case, so we
        // still render a page. The scheme check here is defense in
        // depth: `sanitize_external_link` rejects bad schemes at admin
        // write time, but if the DB column ever gets out of sync
        // (manual edit, future feature) we don't want a hostile
        // `javascript:` value reaching `<a href>`.
        if exhibit.link.is_empty() || !is_safe_href(&exhibit.link) {
            return NavHref {
                href: exhibit.url.clone(),
                open_in_new_tab: false,
            };
        }
        NavHref {
            href: exhibit.link.clone(),
            open_in_new_tab: exhibit.link_target,
        }
    }
    fn intercept(&self, exhibit: &Exhibit) -> Option<HttpResponse> {
        if exhibit.link.is_empty() || !is_safe_href(&exhibit.link) {
            return None;
        }
        Some(
            HttpResponse::Found()
                .append_header(("Location", exhibit.link.as_str()))
                // Admin can change `link` at any time; don't let proxies
                // cache the redirect target.
                .append_header(("Cache-Control", "no-store"))
                .finish(),
        )
    }
    fn render(
        &self,
        exhibit: &Exhibit,
        _content: String,
        _media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        // Fallback page only reached when `link` is empty AND the visitor
        // hit the slug directly. Includes a meta-refresh as belt-and-braces
        // in case JS/headers are stripped by a downstream cache.
        Ok(Page {
            base,
            link: exhibit.link.clone(),
        }
        .render()?)
    }
}

/// Same allowlist as `sanitize_external_link` in the admin save path,
/// but applied at render time so any future code path that writes
/// straight to the `link` column can't smuggle a `javascript:` URL
/// into `<a href>` or a `Location:` header.
fn is_safe_href(raw: &str) -> bool {
    let s = raw.trim();
    if s.is_empty() {
        return false;
    }
    if s.starts_with('/') {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("tel:")
}

#[cfg(test)]
mod tests {
    use super::is_safe_href;

    #[test]
    fn allowed_schemes() {
        for ok in [
            "https://example.com",
            "http://example.com",
            "HTTPS://EXAMPLE.COM",
            "mailto:a@b",
            "tel:+1",
            "/internal/path",
        ] {
            assert!(is_safe_href(ok), "should allow: {ok}");
        }
    }

    #[test]
    fn dangerous_schemes() {
        for bad in [
            "",
            "  ",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,x",
            "vbscript:msgbox",
            "file:///etc/passwd",
            "example.com", // missing scheme
            "../escape",
        ] {
            assert!(!is_safe_href(bad), "should reject: {bad}");
        }
    }
}
