// Image pipeline: shape algorithms + lazy on-demand derivative generation.
//
// Mirrors ndxzstudio/lib/media.php semantics. Originals live in $FILES_DIR/gimgs/
// (uploaded as-is, EXIF-rotated). Derivatives live in $FILES_DIR/dimgs/ and are
// generated on first request to GET /files/dimgs/{ref_id}/{shape}_{size}_{file}.

#![allow(dead_code)] // Used by admin uploads and lazy derivative endpoint (wired in phase 2).

use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Proportional,
    Square,
    FourThree,
    ThreeTwo,
    Cinematic,
}

impl Shape {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "proportional" => Some(Self::Proportional),
            "square" => Some(Self::Square),
            "four_three" => Some(Self::FourThree),
            "three_two" => Some(Self::ThreeTwo),
            "cinematic" => Some(Self::Cinematic),
            _ => None,
        }
    }

    pub fn target_dims(self, max: u32, src_w: u32, src_h: u32) -> (u32, u32) {
        match self {
            Self::Proportional => proportional(max, src_w, src_h),
            Self::Square => (max, max),
            Self::FourThree => (max, max * 3 / 4),
            Self::ThreeTwo => (max, max * 2 / 3),
            Self::Cinematic => (max, max * 9 / 21),
        }
    }
}

fn proportional(max: u32, w: u32, h: u32) -> (u32, u32) {
    let max64 = u64::from(max);
    let w64 = u64::from(w.max(1));
    let h64 = u64::from(h.max(1));
    if w >= h {
        (max, u32::try_from(max64 * h64 / w64).unwrap_or(max))
    } else {
        (u32::try_from(max64 * w64 / h64).unwrap_or(max), max)
    }
}

pub fn load_oriented(path: &Path) -> AppResult<DynamicImage> {
    // actix-multipart writes tempfiles without an extension, so the default
    // `image::open` (which sniffs format from extension only) errors out with
    // "image format could not be determined". with_guessed_format re-reads
    // the magic bytes instead.
    let img = image::ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?;
    let orientation = read_exif_orientation(path).unwrap_or(1);
    Ok(apply_orientation(img, orientation))
}

fn read_exif_orientation(path: &Path) -> Option<u32> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(&file);
    let exif = exif::Reader::new().read_from_container(&mut reader).ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0)
}

fn apply_orientation(img: DynamicImage, orientation: u32) -> DynamicImage {
    match orientation {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

pub fn resize(img: &DynamicImage, shape: Shape, max: u32) -> DynamicImage {
    let (w, h) = img.dimensions();
    let (target_w, target_h) = shape.target_dims(max, w, h);
    match shape {
        Shape::Proportional => img.resize(target_w, target_h, FilterType::Lanczos3),
        _ => img.resize_to_fill(target_w, target_h, FilterType::Lanczos3),
    }
}

pub fn derivative_path(
    files_dir: &Path,
    ref_id: i32,
    shape: Shape,
    size: u32,
    file: &str,
) -> PathBuf {
    files_dir
        .join("dimgs")
        .join(ref_id.to_string())
        .join(format!("{}_{}_{}", shape_name(shape), size, file))
}

pub fn original_path(files_dir: &Path, ref_id: i32, file: &str) -> PathBuf {
    files_dir.join("gimgs").join(ref_id.to_string()).join(file)
}

fn shape_name(shape: Shape) -> &'static str {
    match shape {
        Shape::Proportional => "proportional",
        Shape::Square => "square",
        Shape::FourThree => "four_three",
        Shape::ThreeTwo => "three_two",
        Shape::Cinematic => "cinematic",
    }
}

pub fn ensure_derivative(
    files_dir: &Path,
    ref_id: i32,
    shape: Shape,
    size: u32,
    file: &str,
) -> AppResult<PathBuf> {
    let derivative = derivative_path(files_dir, ref_id, shape, size, file);
    if derivative.exists() {
        return Ok(derivative);
    }
    let original = original_path(files_dir, ref_id, file);
    if !original.exists() {
        return Err(AppError::NotFound);
    }
    let img = load_oriented(&original)?;
    let resized = resize(&img, shape, size);
    if let Some(parent) = derivative.parent() {
        std::fs::create_dir_all(parent)?;
    }
    resized.save(&derivative)?;
    Ok(derivative)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_oriented_decodes_tempfile_without_extension() {
        // Regression: actix-multipart writes tempfiles with no extension, so
        // image::open's extension-based format guess failed. load_oriented
        // must sniff magic bytes instead.
        let mut png_bytes: Vec<u8> = Vec::new();
        DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([255, 0, 0])))
            .write_to(
                &mut std::io::Cursor::new(&mut png_bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        let path = std::env::temp_dir().join(format!(
            "openexhibit-no-ext-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::File::create(&path)
            .unwrap()
            .write_all(&png_bytes)
            .unwrap();
        let loaded = load_oriented(&path).expect("decode no-extension PNG");
        assert_eq!((loaded.width(), loaded.height()), (2, 2));
        let _ = std::fs::remove_file(&path);
    }
}
