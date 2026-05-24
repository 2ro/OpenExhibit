use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, FormatCapabilities, MediaView};

#[derive(Template)]
#[template(path = "public/formats/no_thumbs.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "no_thumbs"
    }
    fn display_name(&self) -> &'static str {
        "No thumbnails"
    }
    fn description(&self) -> &'static str {
        "Media stacked vertically at full size — no thumbnail grid."
    }
    fn capabilities(&self) -> FormatCapabilities {
        FormatCapabilities {
            uses_thumbs_size: false,
            ..FormatCapabilities::default()
        }
    }
    fn render(
        &self,
        _exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        Ok(Page {
            base,
            content,
            media,
        }
        .render()?)
    }
}
