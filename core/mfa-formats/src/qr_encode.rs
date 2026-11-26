//! Draws a QR code as an SVG picture. The text may be a secret (an `otpauth://` link), so the
//! picture is built here and only the picture leaves the core.

use qrcode::render::svg;
use qrcode::QrCode;
use thiserror::Error;

const MIN_SIDE_PIXELS: u32 = 256;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("the text does not fit in a QR code")]
pub struct QrEncodeError;

/// Black modules on a white square with a quiet zone, so it scans on any theme.
///
/// # Errors
/// If `text` is too long for a QR code.
pub fn qr_svg(text: &str) -> Result<String, QrEncodeError> {
    let code = QrCode::new(text.as_bytes()).map_err(|_| QrEncodeError)?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(MIN_SIDE_PIXELS, MIN_SIDE_PIXELS)
        .quiet_zone(true)
        .dark_color(svg::Color("#000000"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_text_becomes_an_svg_picture() {
        let picture = qr_svg("otpauth://totp/x?secret=JBSWY3DPEHPK3PXP").unwrap();
        assert!(picture.contains("<svg"), "{picture}");
        assert!(picture.contains("#ffffff") && picture.contains("#000000"));
    }

    #[test]
    fn the_picture_does_not_contain_the_text() {
        let picture = qr_svg("JBSWY3DPEHPK3PXP").unwrap();
        assert!(!picture.contains("JBSWY3DPEHPK3PXP"));
    }

    #[test]
    fn a_text_that_is_too_long_is_refused() {
        assert_eq!(qr_svg(&"a".repeat(8000)), Err(QrEncodeError));
    }
}
