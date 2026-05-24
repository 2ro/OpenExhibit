use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, FormatCapabilities, MediaView};

#[derive(Template)]
#[template(path = "public/formats/no_show.html")]
struct Page {
    base: BaseFields,
    content: String,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "no_show"
    }
    fn display_name(&self) -> &'static str {
        "Text only"
    }
    fn description(&self) -> &'static str {
        "Just the exhibit's HTML content, no media gallery."
    }
    fn capabilities(&self) -> FormatCapabilities {
        FormatCapabilities {
            uses_media: false,
            uses_thumbs_size: false,
            ..FormatCapabilities::default()
        }
    }
    fn render(
        &self,
        _exhibit: &Exhibit,
        content: String,
        _media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        Ok(Page { base, content }.render()?)
    }
}
