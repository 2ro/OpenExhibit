use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

#[derive(Template)]
#[template(path = "public/formats/visual_index.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
    titling: i16,
    /// Drives the CSS grid track size via inline `--thumb-size` on the
    /// `<ul class="thumbs">`. Changing the admin's thumbnail-size input
    /// makes the rendered cells actually grow/shrink — without this, the
    /// grid was pinned at minmax(180px, 1fr) regardless of the DB value.
    thumb_size: i32,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "visual_index"
    }
    fn display_name(&self) -> &'static str {
        "Visual index"
    }
    fn description(&self) -> &'static str {
        "Grid of thumbnails with a lightbox. Default for most exhibits."
    }
    fn render(
        &self,
        exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        Ok(Page {
            base,
            content,
            media,
            titling: exhibit.titling,
            thumb_size: exhibit.thumbs.into(),
        }
        .render()?)
    }
}
