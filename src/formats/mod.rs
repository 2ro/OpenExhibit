// Exhibit-format registry.
//
// Each format module under this directory exposes a unit struct `Format`
// that implements `ExhibitFormat`. The registry below is the only thing
// that needs to know about every format — adding a new one is a one-line
// addition to `FORMATS` plus a new module file.
//
// See `docs/exhibit-formats.md` (or the README "Adding an exhibit format"
// section) for the contributor walkthrough.

use actix_web::HttpResponse;

use crate::error::AppResult;
use crate::models::{exhibit::Exhibit, media::Media};

pub mod documenta;
pub mod external_link;
pub mod horizontal;
pub mod no_show;
pub mod no_thumbs;
pub mod over_and_over;
pub mod random_image;
pub mod slideshow;
pub mod tag_display;
pub mod thickbox;
pub mod visual_index;

#[derive(Clone)]
pub struct NavExhibit {
    pub id: i32,
    pub title: String,
    pub is_new: bool,
    /// Resolved navigation href — already computed via the format's
    /// `nav_href` so the template doesn't branch per format.
    pub href: String,
    /// When true the template emits `target="_blank" rel="noopener noreferrer"`.
    pub open_in_new_tab: bool,
}

#[derive(Clone)]
pub struct NavSection {
    pub id: i16,
    pub name: String,
    pub hide_title: bool,
    pub top_exhibit: Option<NavExhibit>,
    pub children: Vec<NavExhibit>,
    pub subsections: Vec<NavSubsection>,
}

#[derive(Clone)]
pub struct NavSubsection {
    pub title: String,
    pub exhibits: Vec<NavExhibit>,
}

pub struct BaseFields {
    pub site_lang: String,
    pub page_title: String,
    pub obj_name: String,
    pub description: Option<String>,
    pub body_kind: String,
    pub section_id: i16,
    pub exhibit_id: i32,
    pub format: String,
    pub obj_itop: String,
    pub obj_ibot: String,
    pub nav_sections: Vec<NavSection>,
    /// Site-wide custom CSS (`settings.custom_css`), rendered first.
    pub site_custom_css: String,
    /// Per-exhibit custom CSS (`exhibits.custom_css`), rendered after the
    /// site-wide block so it can override.
    pub exhibit_custom_css: String,
    /// `:root` overrides emitted from the theme color pickers on
    /// /admin/settings — already normalized to `#rrggbb` or empty.
    pub theme_text_color: String,
    pub theme_bg_color: String,
}

#[derive(Clone)]
pub struct MediaView {
    pub id: i32,
    pub title: String,
    pub caption: String,
    pub file_url: String,
    pub thumb_url: String,
    pub thumb_w: i32,
    pub thumb_h: i32,
    pub width: i32,
    pub height: i32,
    pub exhibit_url: String,
    pub is_video: bool,
    pub is_audio: bool,
    pub prev_id: i32,
    pub next_id: i32,
}

impl MediaView {
    fn from_media(
        m: &Media,
        exhibit_url: &str,
        thumb_size: i32,
        markup: crate::markup::RenderOptions,
    ) -> Self {
        let file_url = format!("/files/gimgs/{}/{}", m.ref_id, m.file);
        let thumb_url = if m.thumb.is_empty() {
            format!(
                "/files/dimgs/{}/proportional_{}_{}",
                m.ref_id, thumb_size, m.file
            )
        } else {
            format!("/files/gimgs/{}/{}", m.ref_id, m.thumb)
        };
        Self {
            id: m.id,
            title: m.title.clone(),
            // Captions go through the same markup pipeline as exhibit
            // content — Markdown + BBCode + sanitized HTML, with the
            // same greentext toggle.
            caption: crate::markup::render_with(&m.caption, markup),
            file_url,
            thumb_url,
            thumb_w: thumb_size,
            thumb_h: thumb_size,
            width: m.width,
            height: m.height,
            exhibit_url: exhibit_url.to_string(),
            is_video: m.is_video(),
            is_audio: m.is_audio(),
            prev_id: m.id,
            next_id: m.id,
        }
    }
}

fn wire_lightbox_links(views: &mut [MediaView]) {
    let ids: Vec<i32> = views.iter().map(|v| v.id).collect();
    let n = ids.len();
    if n == 0 {
        return;
    }
    for (i, v) in views.iter_mut().enumerate() {
        v.prev_id = ids[(i + n - 1) % n];
        v.next_id = ids[(i + 1) % n];
    }
}

// ─── Format trait + registry ─────────────────────────────────────────────────

/// What admin-form affordances a format actually uses. The new-exhibit /
/// edit-exhibit template branches on these to hide irrelevant fields per type.
///
/// Defaults to "behaves like a normal media exhibit" — most formats only
/// need to override the bits that differ.
#[allow(clippy::struct_excessive_bools)] // each bool gates one independent admin-form input.
#[derive(Clone, Copy, Debug)]
pub struct FormatCapabilities {
    pub uses_media: bool,
    pub uses_content: bool,
    pub uses_external_link: bool,
    pub uses_password: bool,
    pub uses_thumbs_size: bool,
    /// Whether the admin should be asked to pick a URL slug. `external_link`
    /// sets this `false`: the slug is irrelevant to the user (nav points at
    /// the external URL, direct visits 302 away) so we auto-generate one
    /// and hide it from the form.
    pub requires_url_slug: bool,
}

impl Default for FormatCapabilities {
    fn default() -> Self {
        Self {
            uses_media: true,
            uses_content: true,
            uses_external_link: false,
            uses_password: true,
            uses_thumbs_size: true,
            requires_url_slug: true,
        }
    }
}

/// Resolved nav link for an exhibit. Defaults to the exhibit's internal URL;
/// `external_link` overrides this to use the `link` column.
pub struct NavHref {
    pub href: String,
    pub open_in_new_tab: bool,
}

/// One pluggable exhibit format. Implementations live in `src/formats/<name>.rs`
/// and are registered in the `FORMATS` slice below.
///
/// Trait is object-safe so the registry can store `&'static dyn ExhibitFormat`.
pub trait ExhibitFormat: Send + Sync {
    /// Stable identifier stored in `exhibits.format`. Lowercase `snake_case`.
    fn key(&self) -> &'static str;

    /// Human-readable name shown in the admin "New exhibit" picker.
    fn display_name(&self) -> &'static str;

    /// One-line description shown next to the picker option.
    fn description(&self) -> &'static str;

    fn capabilities(&self) -> FormatCapabilities {
        FormatCapabilities::default()
    }

    /// Intercept a public visit before any rendering happens. Returning
    /// `Some` short-circuits the request — used by `external_link` to 302
    /// off to the external URL. Default: render normally.
    fn intercept(&self, _exhibit: &Exhibit) -> Option<HttpResponse> {
        None
    }

    /// Compute the nav-bar link for this exhibit. Default: the exhibit's
    /// own internal URL, opening in the same tab.
    fn nav_href(&self, exhibit: &Exhibit) -> NavHref {
        NavHref {
            href: exhibit.url.clone(),
            open_in_new_tab: false,
        }
    }

    /// Render the public exhibit page. Almost always delegates to an
    /// Askama template. `content` is the exhibit's `content` column
    /// already passed through the markup pipeline (Markdown + `BBCode` +
    /// sanitized HTML), so the format just slots it into the template.
    fn render(
        &self,
        exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String>;
}

/// The registry. To add a new format: add a `pub mod` above, create the
/// `Format` struct in that module, append `&<name>::Format` here.
static FORMATS: &[&dyn ExhibitFormat] = &[
    &visual_index::Format,
    &slideshow::Format,
    &horizontal::Format,
    &no_show::Format,
    &no_thumbs::Format,
    &over_and_over::Format,
    &random_image::Format,
    &thickbox::Format,
    &documenta::Format,
    &tag_display::Format,
    &external_link::Format,
];

/// All registered formats, in admin-picker display order.
pub fn registry() -> &'static [&'static dyn ExhibitFormat] {
    FORMATS
}

/// Look up a format by its stored `key`. Unknown keys fall back to
/// `visual_index` so an exhibit with a stale format value still renders.
pub fn find(key: &str) -> &'static dyn ExhibitFormat {
    FORMATS
        .iter()
        .copied()
        .find(|f| f.key() == key)
        .unwrap_or(&visual_index::Format)
}

/// Top-level render entry point called from `routes::public`. `greentext`
/// flips the markup pipeline's `>`-line behavior; everything else is the
/// same Markdown + `BBCode` + sanitized HTML pipeline.
pub fn render(
    exhibit: &Exhibit,
    media: &[Media],
    base: BaseFields,
    greentext: bool,
) -> AppResult<String> {
    let markup_opts = crate::markup::RenderOptions { greentext };
    let mut media_views: Vec<MediaView> = media
        .iter()
        .map(|m| MediaView::from_media(m, &exhibit.url, exhibit.thumbs.into(), markup_opts))
        .collect();
    wire_lightbox_links(&mut media_views);
    let content = crate::markup::render_with(&exhibit.content, markup_opts);
    find(&exhibit.format).render(exhibit, content, media_views, base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn registry_keys_are_unique() {
        let mut seen = HashSet::new();
        for f in registry() {
            assert!(seen.insert(f.key()), "duplicate format key: {}", f.key());
        }
    }

    #[test]
    fn registry_has_visual_index() {
        // visual_index is the fallback for unknown keys, so it MUST be present.
        assert!(registry().iter().any(|f| f.key() == "visual_index"));
    }

    #[test]
    fn unknown_key_falls_back_to_visual_index() {
        assert_eq!(find("nonexistent").key(), "visual_index");
    }

    #[test]
    fn display_names_are_nonempty() {
        for f in registry() {
            assert!(
                !f.display_name().is_empty(),
                "{}: empty display name",
                f.key()
            );
            assert!(
                !f.description().is_empty(),
                "{}: empty description",
                f.key()
            );
        }
    }
}
