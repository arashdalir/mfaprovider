//! Reading QR codes out of PNG and JPEG images (ADR-0013). The caller passes the file's
//! bytes; nothing here touches the filesystem. Input is untrusted, so every limit is checked
//! from the image header before any pixel buffer is allocated.

use std::io::Cursor;

use png::{ColorType, Decoder, Limits, Transformations};
use rqrr::PreparedImage;
use thiserror::Error;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::JpegDecoder;

pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const MAX_SIDE: u32 = 8192;
pub const MAX_PIXELS: u64 = 25_000_000;
pub const PNG_MEMORY_LIMIT: usize = 128 * 1024 * 1024;
pub const MAX_CODES: usize = 16;

const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";
const JPEG_MAGIC: &[u8] = &[0xFF, 0xD8, 0xFF];

/// Why an image could not be read. The messages are fixed sentences: they never include file
/// contents or decoded text.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum QrImageError {
    #[error("the file is larger than 10 MiB")]
    FileTooLarge,
    #[error("the file is not a PNG or JPEG image")]
    UnsupportedFormat,
    #[error("the image is damaged or cannot be read")]
    Damaged,
    #[error("the image is too large (at most 5000 × 5000 pixels)")]
    ImageTooLarge,
    #[error("no QR code was found in the image")]
    NoQrCode,
}

struct Grey {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
}

/// The text of every QR code found in the image, in detection order, without repeats.
///
/// # Errors
/// See [`QrImageError`].
pub fn decode_qr_image(bytes: &[u8]) -> Result<Vec<String>, QrImageError> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(QrImageError::FileTooLarge);
    }
    let grey = if bytes.starts_with(PNG_MAGIC) {
        decode_png(bytes)?
    } else if bytes.starts_with(JPEG_MAGIC) {
        decode_jpeg(bytes)?
    } else {
        return Err(QrImageError::UnsupportedFormat);
    };
    find_codes(&grey)
}

fn find_codes(grey: &Grey) -> Result<Vec<String>, QrImageError> {
    let mut prepared = PreparedImage::prepare_from_greyscale(grey.width, grey.height, |x, y| {
        grey.pixels[y * grey.width + x]
    });
    let mut texts: Vec<String> = Vec::new();
    for grid in prepared.detect_grids() {
        if let Ok((_, text)) = grid.decode() {
            if !texts.contains(&text) {
                texts.push(text);
            }
        }
        if texts.len() == MAX_CODES {
            break;
        }
    }
    if texts.is_empty() {
        return Err(QrImageError::NoQrCode);
    }
    Ok(texts)
}

fn check_dimensions(width: u64, height: u64) -> Result<(), QrImageError> {
    if width == 0 || height == 0 {
        return Err(QrImageError::Damaged);
    }
    let side = u64::from(MAX_SIDE);
    if width > side || height > side || width * height > MAX_PIXELS {
        return Err(QrImageError::ImageTooLarge);
    }
    Ok(())
}

fn decode_png(bytes: &[u8]) -> Result<Grey, QrImageError> {
    let mut decoder = Decoder::new_with_limits(
        Cursor::new(bytes),
        Limits {
            bytes: PNG_MEMORY_LIMIT,
        },
    );
    decoder.set_transformations(Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|_| QrImageError::Damaged)?;
    let (width, height) = (reader.info().width, reader.info().height);
    check_dimensions(width.into(), height.into())?;
    let size = reader
        .output_buffer_size()
        .filter(|size| *size <= PNG_MEMORY_LIMIT)
        .ok_or(QrImageError::ImageTooLarge)?;
    let mut buffer = vec![0_u8; size];
    let frame = reader
        .next_frame(&mut buffer)
        .map_err(|_| QrImageError::Damaged)?;
    let channels = match frame.color_type {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => return Err(QrImageError::Damaged),
    };
    let used = buffer
        .get(..frame.buffer_size())
        .ok_or(QrImageError::Damaged)?;
    let pixels = used
        .chunks_exact(channels)
        .map(|pixel| flatten_onto_white(frame.color_type, pixel))
        .collect();
    Ok(Grey {
        width: width as usize,
        height: height as usize,
        pixels,
    })
}

/// Luminance of one pixel; transparency is blended with a white background, so a code drawn
/// in opaque dark modules on a transparent page still reads as dark on light.
fn flatten_onto_white(color_type: ColorType, pixel: &[u8]) -> u8 {
    let (luma, alpha) = match color_type {
        ColorType::Grayscale => (u32::from(pixel[0]), 255),
        ColorType::GrayscaleAlpha => (u32::from(pixel[0]), u32::from(pixel[1])),
        ColorType::Rgb => (luminance(pixel), 255),
        _ => (luminance(pixel), u32::from(pixel[3])),
    };
    let blended = (luma * alpha + 255 * (255 - alpha)) / 255;
    u8::try_from(blended).unwrap_or(u8::MAX)
}

fn luminance(rgb: &[u8]) -> u32 {
    (299 * u32::from(rgb[0]) + 587 * u32::from(rgb[1]) + 114 * u32::from(rgb[2])) / 1000
}

fn decode_jpeg(bytes: &[u8]) -> Result<Grey, QrImageError> {
    // The decoder's own size limits are lifted to the format maximum so that our check below
    // decides; no pixel buffer exists until that check has passed.
    let format_maximum = usize::from(u16::MAX);
    let options = DecoderOptions::default()
        .set_max_width(format_maximum)
        .set_max_height(format_maximum)
        .jpeg_set_out_colorspace(ColorSpace::Luma);
    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(bytes), options);
    decoder
        .decode_headers()
        .map_err(|_| QrImageError::Damaged)?;
    let info = decoder.info().ok_or(QrImageError::Damaged)?;
    check_dimensions(info.width.into(), info.height.into())?;
    let pixels = decoder.decode().map_err(|_| QrImageError::Damaged)?;
    let (width, height) = (usize::from(info.width), usize::from(info.height));
    if pixels.len() != width * height {
        return Err(QrImageError::Damaged);
    }
    Ok(Grey {
        width,
        height,
        pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::otpauth::parse_otpauth;
    use mfa_core::otp::OtpRegistry;
    use qrcode::{Color, QrCode};

    const TEXT: &str =
        "otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example";

    /// A QR code as a grey pixel grid: dark modules 0, light 255, four-module quiet zone.
    fn qr_grey(text: &str, scale: usize) -> (usize, usize, Vec<u8>) {
        let code = QrCode::new(text.as_bytes()).unwrap();
        let modules = code.width();
        let colours = code.to_colors();
        let side = (modules + 8) * scale;
        let mut pixels = vec![255_u8; side * side];
        for y in 0..side {
            for x in 0..side {
                let (mx, my) = (x / scale, y / scale);
                let inside = (4..modules + 4).contains(&mx) && (4..modules + 4).contains(&my);
                if inside && colours[(my - 4) * modules + (mx - 4)] == Color::Dark {
                    pixels[y * side + x] = 0;
                }
            }
        }
        (side, side, pixels)
    }

    fn png_bytes(
        width: usize,
        height: usize,
        color: png::ColorType,
        depth: png::BitDepth,
        palette: Option<&[u8]>,
        data: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(
            &mut out,
            u32::try_from(width).unwrap(),
            u32::try_from(height).unwrap(),
        );
        encoder.set_color(color);
        encoder.set_depth(depth);
        if let Some(palette) = palette {
            encoder.set_palette(palette.to_vec());
        }
        encoder
            .write_header()
            .unwrap()
            .write_image_data(data)
            .unwrap();
        out
    }

    fn grey_png(width: usize, height: usize, pixels: &[u8]) -> Vec<u8> {
        png_bytes(
            width,
            height,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            None,
            pixels,
        )
    }

    fn qr_png(text: &str) -> Vec<u8> {
        let (w, h, pixels) = qr_grey(text, 6);
        grey_png(w, h, &pixels)
    }

    fn qr_jpeg(text: &str) -> Vec<u8> {
        let (w, h, pixels) = qr_grey(text, 6);
        let mut out = Vec::new();
        jpeg_encoder::Encoder::new(&mut out, 90)
            .encode(
                &pixels,
                u16::try_from(w).unwrap(),
                u16::try_from(h).unwrap(),
                jpeg_encoder::ColorType::Luma,
            )
            .unwrap();
        out
    }

    fn texts(bytes: &[u8]) -> Vec<String> {
        decode_qr_image(bytes).unwrap_or_else(|error| panic!("{error}"))
    }

    #[test]
    fn a_png_with_a_qr_code_is_read() {
        assert_eq!(texts(&qr_png(TEXT)), [TEXT]);
    }

    #[test]
    fn a_jpeg_with_a_qr_code_is_read() {
        assert_eq!(texts(&qr_jpeg(TEXT)), [TEXT]);
    }

    #[test]
    fn colour_and_transparent_pngs_are_read() {
        let (w, h, grey) = qr_grey(TEXT, 6);
        let rgb: Vec<u8> = grey.iter().flat_map(|v| [*v, *v, *v]).collect();
        let opaque_png = png_bytes(w, h, png::ColorType::Rgb, png::BitDepth::Eight, None, &rgb);
        assert_eq!(texts(&opaque_png), [TEXT]);

        let rgba: Vec<u8> = grey
            .iter()
            .flat_map(|v| {
                if *v == 0 {
                    [0, 0, 0, 255]
                } else {
                    [0, 0, 0, 0]
                }
            })
            .collect();
        let transparent_png = png_bytes(
            w,
            h,
            png::ColorType::Rgba,
            png::BitDepth::Eight,
            None,
            &rgba,
        );
        assert_eq!(
            texts(&transparent_png),
            [TEXT],
            "dark modules on a transparent page"
        );

        let grey_alpha: Vec<u8> = grey.iter().flat_map(|v| [*v, 255]).collect();
        let ga_png = png_bytes(
            w,
            h,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            None,
            &grey_alpha,
        );
        assert_eq!(texts(&ga_png), [TEXT]);
    }

    #[test]
    fn palette_and_sixteen_bit_pngs_are_read() {
        let (w, h, grey) = qr_grey(TEXT, 6);
        let indices: Vec<u8> = grey.iter().map(|v| u8::from(*v != 0)).collect();
        let palette = [0, 0, 0, 255, 255, 255];
        let indexed = png_bytes(
            w,
            h,
            png::ColorType::Indexed,
            png::BitDepth::Eight,
            Some(&palette),
            &indices,
        );
        assert_eq!(texts(&indexed), [TEXT]);

        let sixteen: Vec<u8> = grey.iter().flat_map(|v| [*v, *v]).collect();
        let deep = png_bytes(
            w,
            h,
            png::ColorType::Grayscale,
            png::BitDepth::Sixteen,
            None,
            &sixteen,
        );
        assert_eq!(texts(&deep), [TEXT]);
    }

    #[test]
    fn several_codes_in_one_image_are_all_returned() {
        let (w, h, a) = qr_grey("first code", 6);
        let (_, _, b) = qr_grey("second code", 6);
        let mut pixels = Vec::new();
        for row in 0..h {
            pixels.extend_from_slice(&a[row * w..(row + 1) * w]);
            pixels.extend_from_slice(&b[row * w..(row + 1) * w]);
        }
        let mut found = texts(&grey_png(w * 2, h, &pixels));
        found.sort();
        assert_eq!(found, ["first code", "second code"]);
    }

    #[test]
    fn an_otpauth_code_goes_straight_into_an_entry() {
        let text = texts(&qr_png(TEXT)).remove(0);
        let entry = parse_otpauth(&text, &OtpRegistry::with_builtins()).unwrap();
        assert_eq!(
            (entry.issuer.as_str(), entry.account.as_str()),
            ("Example", "alice@example.com")
        );
    }

    #[test]
    fn an_image_without_a_code_says_so() {
        assert_eq!(
            decode_qr_image(&grey_png(64, 64, &[255; 64 * 64])),
            Err(QrImageError::NoQrCode)
        );
        let noise: Vec<u8> = (0..200 * 200_u32)
            .map(|n| u8::try_from(n.wrapping_mul(2_654_435_761) >> 24).unwrap())
            .collect();
        assert_eq!(
            decode_qr_image(&grey_png(200, 200, &noise)),
            Err(QrImageError::NoQrCode)
        );
    }

    #[test]
    fn empty_and_foreign_input_is_refused() {
        assert_eq!(decode_qr_image(&[]), Err(QrImageError::UnsupportedFormat));
        assert_eq!(
            decode_qr_image(b"GIF89a....."),
            Err(QrImageError::UnsupportedFormat)
        );
        assert_eq!(
            decode_qr_image(b"otpauth://totp/me?secret=JBSWY3DP"),
            Err(QrImageError::UnsupportedFormat)
        );
        let noise: Vec<u8> = (0..4096_u32)
            .map(|n| u8::try_from(n.wrapping_mul(2_246_822_519) >> 24).unwrap())
            .collect();
        assert_eq!(
            decode_qr_image(&noise),
            Err(QrImageError::UnsupportedFormat)
        );
    }

    #[test]
    fn a_bare_magic_number_is_a_damaged_image() {
        assert_eq!(decode_qr_image(PNG_MAGIC), Err(QrImageError::Damaged));
        assert_eq!(decode_qr_image(JPEG_MAGIC), Err(QrImageError::Damaged));
    }

    #[test]
    fn truncated_files_never_panic_and_never_invent_a_code() {
        for original in [qr_png(TEXT), qr_jpeg(TEXT)] {
            for tenths in 1..10 {
                let cut = original.len() * tenths / 10;
                let outcome = decode_qr_image(&original[..cut]);
                assert!(
                    outcome.is_err() || outcome == Ok(vec![TEXT.to_owned()]),
                    "cut at {cut}: {outcome:?}"
                );
            }
        }
    }

    #[test]
    fn flipping_bytes_never_panics() {
        for original in [qr_png(TEXT), qr_jpeg(TEXT)] {
            let step = (original.len() / 150).max(1);
            for position in (0..original.len()).step_by(step) {
                for mask in [0x01, 0x80, 0xFF] {
                    let mut damaged = original.clone();
                    damaged[position] ^= mask;
                    let _ = decode_qr_image(&damaged);
                }
            }
        }
    }

    fn png_chunk(kind: [u8; 4], body: &[u8]) -> Vec<u8> {
        let mut checked = kind.to_vec();
        checked.extend_from_slice(body);
        let mut chunk = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
        chunk.extend_from_slice(&checked);
        chunk.extend_from_slice(&crc32fast::hash(&checked).to_be_bytes());
        chunk
    }

    /// A tiny PNG whose header claims `width` × `height` but whose image data is empty.
    fn png_header_only(width: u32, height: u32) -> Vec<u8> {
        let mut header = width.to_be_bytes().to_vec();
        header.extend_from_slice(&height.to_be_bytes());
        header.extend_from_slice(&[8, 0, 0, 0, 0]);
        let mut file = Vec::from(PNG_MAGIC);
        file.extend(png_chunk(*b"IHDR", &header));
        file.extend(png_chunk(
            *b"IDAT",
            &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
        ));
        file.extend(png_chunk(*b"IEND", &[]));
        file
    }

    #[test]
    fn a_tiny_png_claiming_a_huge_size_is_refused_before_anything_is_allocated() {
        assert_eq!(
            decode_qr_image(&png_header_only(100_000, 100_000)),
            Err(QrImageError::ImageTooLarge)
        );
        assert_eq!(
            decode_qr_image(&png_header_only(MAX_SIDE + 1, 1)),
            Err(QrImageError::ImageTooLarge)
        );
        assert_eq!(
            decode_qr_image(&png_header_only(6000, 6000)),
            Err(QrImageError::ImageTooLarge),
            "36 million pixels"
        );
    }

    #[test]
    fn a_png_with_no_pixels_is_damaged() {
        assert_eq!(
            decode_qr_image(&png_header_only(0, 10)),
            Err(QrImageError::Damaged)
        );
    }

    #[test]
    fn a_jpeg_claiming_a_huge_size_is_refused() {
        let mut jpeg = qr_jpeg(TEXT);
        let sof = jpeg
            .windows(2)
            .position(|pair| pair == [0xFF, 0xC0])
            .expect("a baseline frame header");
        jpeg[sof + 5..sof + 9].copy_from_slice(&[0xEA, 0x60, 0xEA, 0x60]);
        assert_eq!(decode_qr_image(&jpeg), Err(QrImageError::ImageTooLarge));
    }

    #[test]
    fn files_over_the_size_limit_are_refused_without_decoding() {
        let mut big = Vec::from(PNG_MAGIC);
        big.resize(MAX_FILE_BYTES + 1, 0);
        assert_eq!(decode_qr_image(&big), Err(QrImageError::FileTooLarge));
    }

    #[test]
    fn the_largest_allowed_image_is_still_read() {
        let side = 3000;
        let (w, h, pixels) = qr_grey(TEXT, 3000 / 49);
        assert!(w <= side && h <= side);
        let mut canvas = vec![255_u8; side * side];
        for row in 0..h {
            canvas[row * side..row * side + w].copy_from_slice(&pixels[row * w..(row + 1) * w]);
        }
        assert_eq!(texts(&grey_png(side, side, &canvas)), [TEXT]);
    }

    #[test]
    fn error_messages_are_fixed_sentences() {
        for error in [
            QrImageError::FileTooLarge,
            QrImageError::UnsupportedFormat,
            QrImageError::Damaged,
            QrImageError::ImageTooLarge,
            QrImageError::NoQrCode,
        ] {
            let message = error.to_string();
            assert!(
                message.chars().all(|c| c != '\n') && message.len() < 80,
                "{message}"
            );
        }
    }
}
