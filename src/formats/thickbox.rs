use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

#[derive(Template)]
#[template(path = "public/formats/thickbox.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "thickbox"
    }
    fn display_name(&self) -> &'static str {
        "Thickbox"
    }
    fn description(&self) -> &'static str {
        "Classic thumbnail-to-lightbox layout."
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
