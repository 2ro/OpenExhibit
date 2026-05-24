use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

#[derive(Template)]
#[template(path = "public/formats/horizontal.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "horizontal"
    }
    fn display_name(&self) -> &'static str {
        "Horizontal strip"
    }
    fn description(&self) -> &'static str {
        "Side-scrolling row of full-height images."
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
