use askama::Template;
use rand::seq::SliceRandom;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, MediaView};

#[derive(Template)]
#[template(path = "public/formats/random_image.html")]
struct Page {
    base: BaseFields,
    content: String,
    chosen: Option<MediaView>,
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str {
        "random_image"
    }
    fn display_name(&self) -> &'static str {
        "Random image"
    }
    fn description(&self) -> &'static str {
        "Shows a single random image from the gallery, picked per page load."
    }
    fn render(
        &self,
        _exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        let chosen = media.choose(&mut rand::thread_rng()).cloned();
        Ok(Page {
            base,
            content,
            chosen,
        }
        .render()?)
    }
}
