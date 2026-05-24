use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

#[derive(Template)]
#[template(path = "public/formats/over_and_over.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "over_and_over"
    }
    fn display_name(&self) -> &'static str {
        "Over and over"
    }
    fn description(&self) -> &'static str {
        "Vertically stacked images that loop on long scrolls."
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
