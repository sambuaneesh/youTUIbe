use directories::BaseDirs;
use image::imageops::FilterType;
use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};
use tokio::process::Command;

const THUMBNAILER: &str = r#"[Thumbnailer Entry]
TryExec=ffmpegthumbnailer
Exec=ffmpegthumbnailer -s %s -i %i -o %o -c png -t 10
MimeType=audio/x-opus+ogg;audio/opus;audio/ogg;
"#;

/// Register the MIME type Nautilus uses for `.opus`. Many distributions ship an
/// audio thumbnailer but omit `audio/x-opus+ogg`, even though the same tool can
/// read the embedded METADATA_BLOCK_PICTURE perfectly.
pub fn ensure_registration() -> Result<(), String> {
    if !crate::ytdlp::executable_exists("ffmpegthumbnailer") {
        return Ok(());
    }
    let base = BaseDirs::new().ok_or_else(|| "user data directory unavailable".to_string())?;
    let directory = base.data_local_dir().join("thumbnailers");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("create thumbnailer directory: {error}"))?;
    let path = directory.join("youtuibe-opus.thumbnailer");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(THUMBNAILER) {
        std::fs::write(&path, THUMBNAILER)
            .map_err(|error| format!("register Opus artwork thumbnailer: {error}"))?;
    }
    Ok(())
}

/// Prime freedesktop.org thumbnail caches after a completed audio download.
/// This is only presentation metadata: failure never changes job success.
pub async fn generate(path: PathBuf) -> Result<(), String> {
    if !path.is_file() || !crate::ytdlp::executable_exists("ffmpegthumbnailer") {
        return Ok(());
    }
    let uri = desktop_file_uri(&path)?;
    let modified = path
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |duration| duration.as_secs());
    let hash = format!("{:x}", md5::compute(uri.as_bytes()));
    let base = BaseDirs::new().ok_or_else(|| "user cache directory unavailable".to_string())?;
    let root = base.cache_dir().join("thumbnails");
    std::fs::create_dir_all(&root).map_err(|error| format!("create thumbnail cache: {error}"))?;
    let raw = root.join(format!(".youtuibe-{hash}-{}.png", uuid::Uuid::new_v4()));
    let output = Command::new("ffmpegthumbnailer")
        .args(["-s", "512", "-i"])
        .arg(&path)
        .args(["-o"])
        .arg(&raw)
        .args(["-c", "png", "-t", "10"])
        .output()
        .await
        .map_err(|error| format!("start artwork thumbnailer: {error}"))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&raw);
        return Err(format!(
            "artwork thumbnailer exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let source = image::open(&raw).map_err(|error| format!("decode artwork cache: {error}"))?;
    let _ = std::fs::remove_file(&raw);
    for (directory, size) in [("normal", 128_u32), ("large", 256_u32)] {
        let directory = root.join(directory);
        std::fs::create_dir_all(&directory)
            .map_err(|error| format!("create {size}px thumbnail cache: {error}"))?;
        let image = contain_square(&source, size);
        write_thumbnail(
            &directory.join(format!("{hash}.png")),
            &image,
            &uri,
            modified,
        )?;
    }
    Ok(())
}

fn contain_square(source: &image::DynamicImage, size: u32) -> image::RgbImage {
    let fitted = source.resize(size, size, FilterType::Lanczos3).to_rgb8();
    let mut canvas = image::RgbImage::from_pixel(size, size, image::Rgb([7, 11, 17]));
    let x = i64::from((size - fitted.width()) / 2);
    let y = i64::from((size - fitted.height()) / 2);
    image::imageops::overlay(&mut canvas, &fitted, x, y);
    canvas
}

fn desktop_file_uri(path: &Path) -> Result<String, String> {
    Ok(url::Url::from_file_path(path)
        .map_err(|_| format!("cannot form file URI for {}", path.display()))?
        .to_string()
        // GLib (and therefore Nautilus) percent-encodes brackets in file URIs,
        // while the WHATWG URL serializer leaves them literal. Cache identity
        // must match GLib byte-for-byte.
        .replace('[', "%5B")
        .replace(']', "%5D"))
}

fn write_thumbnail(
    path: &Path,
    image: &image::RgbImage,
    uri: &str,
    modified: u64,
) -> Result<(), String> {
    let temporary = path.with_extension(format!("png.{}.tmp", uuid::Uuid::new_v4()));
    let file = File::create(&temporary)
        .map_err(|error| format!("create thumbnail {}: {error}", temporary.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), image.width(), image.height());
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk("Thumb::URI".into(), uri.into())
        .map_err(|error| format!("write thumbnail URI: {error}"))?;
    encoder
        .add_text_chunk("Thumb::MTime".into(), modified.to_string())
        .map_err(|error| format!("write thumbnail timestamp: {error}"))?;
    encoder
        .add_text_chunk("Software".into(), "youTUIbe".into())
        .map_err(|error| format!("write thumbnail creator: {error}"))?;
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("write thumbnail header: {error}"))?;
    writer
        .write_image_data(image.as_raw())
        .map_err(|error| format!("write thumbnail pixels: {error}"))?;
    writer
        .finish()
        .map_err(|error| format!("finish thumbnail: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish thumbnail {}: {error}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::BufReader as StdBufReader;

    #[test]
    fn cached_png_contains_freedesktop_identity() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("thumb.png");
        let image = image::RgbImage::from_pixel(2, 2, image::Rgb([10, 20, 30]));
        write_thumbnail(&path, &image, "file:///tmp/song.opus", 1234).unwrap();

        let decoder = png::Decoder::new(StdBufReader::new(File::open(path).unwrap()));
        let reader = decoder.read_info().unwrap();
        let chunks = &reader.info().uncompressed_latin1_text;
        assert!(chunks.iter().any(|chunk| {
            chunk.keyword == "Thumb::URI" && chunk.text == "file:///tmp/song.opus"
        }));
        assert!(
            chunks
                .iter()
                .any(|chunk| chunk.keyword == "Thumb::MTime" && chunk.text == "1234")
        );
    }

    #[test]
    fn desktop_uri_matches_glib_bracket_encoding() {
        let uri = desktop_file_uri(Path::new("/tmp/Song [abc].opus")).unwrap();
        assert_eq!(uri, "file:///tmp/Song%20%5Babc%5D.opus");
    }

    #[test]
    fn rectangular_art_is_contained_without_cropping_or_stretching() {
        let source = image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            16,
            9,
            image::Rgb([200, 100, 50]),
        ));
        let result = contain_square(&source, 32);
        assert_eq!(result.dimensions(), (32, 32));
        assert_eq!(result.get_pixel(16, 0), &image::Rgb([7, 11, 17]));
        assert_eq!(result.get_pixel(16, 16), &image::Rgb([200, 100, 50]));
    }
}
