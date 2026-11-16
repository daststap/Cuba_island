//! Clipboard images on disk: full PNG + 256 px preview in `app_data_dir/clips/`.

use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};

use crate::error::{AppError, AppResult};

pub const THUMB_SIDE: u32 = 256;

fn image_error(err: image::ImageError) -> AppError {
    AppError::Internal(format!("ошибка изображения: {err}"))
}

/// File name of the preview for a stored image.
pub fn thumb_name(image_name: &str) -> String {
    format!("{}.thumb.png", image_name.trim_end_matches(".png"))
}

/// Saves RGBA pixels as `<stem>.png` plus its preview. Returns the PNG file name.
pub fn save(dir: &Path, stem: &str, width: u32, height: u32, rgba: Vec<u8>) -> AppResult<String> {
    let image = RgbaImage::from_raw(width, height, rgba).ok_or_else(|| {
        AppError::InvalidArgument("размер картинки не совпадает с данными".into())
    })?;
    std::fs::create_dir_all(dir)?;
    let name = format!("{stem}.png");
    image
        .save_with_format(dir.join(&name), ImageFormat::Png)
        .map_err(image_error)?;

    let thumb = if width > THUMB_SIDE || height > THUMB_SIDE {
        image::DynamicImage::ImageRgba8(image)
            .resize(THUMB_SIDE, THUMB_SIDE, FilterType::Triangle)
            .to_rgba8()
    } else {
        image
    };
    thumb
        .save_with_format(dir.join(thumb_name(&name)), ImageFormat::Png)
        .map_err(image_error)?;
    Ok(name)
}

/// Loads a stored PNG as RGBA pixels: `(width, height, bytes)`.
pub fn load(dir: &Path, name: &str) -> AppResult<(u32, u32, Vec<u8>)> {
    let image = image::open(dir.join(name)).map_err(image_error)?.to_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

/// Removes a stored PNG and its preview; missing files are fine.
pub fn remove(dir: &Path, name: &str) {
    for file in [PathBuf::from(name), PathBuf::from(thumb_name(name))] {
        if let Err(err) = std::fs::remove_file(dir.join(&file)) {
            if err.kind() != std::io::ErrorKind::NotFound {
                log::warn!("failed to remove {}: {err}", file.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cuba-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn saves_loads_and_removes_with_preview() {
        let dir = temp_dir("images");
        let (w, h) = (600, 300);
        let pixels: Vec<u8> = (0..w * h)
            .flat_map(|i| [(i % 256) as u8, 0, 0, 255])
            .collect();

        let name = save(&dir, "abc", w, h, pixels.clone()).unwrap();
        assert_eq!(name, "abc.png");
        assert_eq!(load(&dir, &name).unwrap(), (w, h, pixels));

        let thumb = image::open(dir.join(thumb_name(&name))).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (256, 128));

        remove(&dir, &name);
        assert!(!dir.join(&name).exists());
        assert!(!dir.join(thumb_name(&name)).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_mismatched_pixel_data() {
        let dir = temp_dir("bad");
        assert!(save(&dir, "x", 10, 10, vec![0; 3]).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
