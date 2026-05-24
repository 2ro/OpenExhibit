use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

/// How long each slide is fully visible, in seconds. Total cycle is
/// `SLIDE_SECONDS * media.len()`.
const SLIDE_SECONDS: f64 = 5.0;
/// Cross-fade duration in seconds. Eats into both ends of each slide's
/// visible window.
const FADE_SECONDS: f64 = 0.45;

#[derive(Template)]
#[template(path = "public/formats/slideshow.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
    /// `true` if autoplay should be wired up. Two-or-more slides only —
    /// a one-slide deck has nothing to cycle through.
    autoplay_supported: bool,
    /// Total animation duration in seconds: `media.len() * SLIDE_SECONDS`.
    /// Same for every slide; per-slide offset is set via `animation-delay`.
    total_seconds: f64,
    /// Per-slide visible duration in seconds (== `SLIDE_SECONDS`).
    /// Templated into each slide's `animation-delay: calc(... * index)`.
    slide_seconds: f64,
    /// Keyframe-percentage stops, pre-computed so the template doesn't
    /// have to do floating-point in Askama. Sequence is:
    ///   `0%`                 → opacity 0
    ///   `fade_in_pct`        → opacity 1   (faded in)
    ///   `visible_end_pct`    → opacity 1   (start fading out)
    ///   `fade_out_end_pct`   → opacity 0   (gone)
    ///   `100%`               → opacity 0   (waits for next cycle)
    fade_in_pct: f64,
    visible_end_pct: f64,
    fade_out_end_pct: f64,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "slideshow"
    }
    fn display_name(&self) -> &'static str {
        "Slideshow"
    }
    fn description(&self) -> &'static str {
        "One slide at a time, paginated. Good for sequential narratives."
    }
    fn render(
        &self,
        _exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        // Recompute timing per render so it tracks the actual media count.
        // Single-slide decks skip the autoplay machinery entirely.
        #[allow(clippy::cast_precision_loss)]
        let n = media.len() as f64;
        let total = (n.max(1.0)) * SLIDE_SECONDS;
        let fade_in_pct = (FADE_SECONDS / total) * 100.0;
        let visible_end_pct = ((SLIDE_SECONDS - FADE_SECONDS) / total) * 100.0;
        let fade_out_end_pct = (SLIDE_SECONDS / total) * 100.0;
        let autoplay_supported = media.len() >= 2;

        Ok(Page {
            base,
            content,
            media,
            autoplay_supported,
            total_seconds: total,
            slide_seconds: SLIDE_SECONDS,
            fade_in_pct,
            visible_end_pct,
            fade_out_end_pct,
        }
        .render()?)
    }
}
