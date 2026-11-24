# ADR-0013: QR image decoding — `rqrr` + `png` + `zune-jpeg`, with hard input limits

- Status: accepted (2026-11-24, by the user, as written)
- Date: 2026-11-24
- Related tasks: TASK-0009 (also TASK-0010 for Google migration QR codes, TASK-0012 for camera frames)
- Refines: ADR-0004 ("otpauth:// URIs and QR images"), ADR-0002 ("seeds never enter JavaScript")

## Context
Adding an account from a screenshot or photo of a QR code means the app opens an image file chosen by the user, which is untrusted input, and the code inside it contains the seed. Decoding must happen in Rust so the seed never reaches JavaScript (ADR-0002), and a hostile or accidental file (a decompression bomb, a truncated file, random bytes) must not hang the app or exhaust memory.

Candidates, checked on crates.io on 2026-11-24:

| | Role | Version / last update | Direct dependencies | Licence |
|---|---|---|---|---|
| `rqrr` | find and decode QR codes in a greyscale pixel grid | 0.11.0, 2026-09-02 | `g2p`, `lru` (`image` only as an optional feature) | (MIT OR Apache-2.0) AND ISC |
| `png` | PNG decoder with configurable limits | 0.18.1, 2026-02-14 | `bitflags`, `crc32fast`, `fdeflate`, `flate2`, `miniz_oxide` | MIT OR Apache-2.0 |
| `zune-jpeg` | JPEG decoder with dimension limits | 0.5.15, 2026-10-19 | `zune-core` | MIT OR Apache-2.0 OR Zlib |
| `image` | umbrella decoder crate | 0.25.10, 2026-03-10 | always pulls `bytemuck`, `moxcms`, `num-traits`, `byteorder-lite`, plus the codecs above | MIT OR Apache-2.0 |
| `rxing` | full ZXing port (many barcode types) | 0.9.4, 2026-11-08 | large | Apache-2.0 |
| `quircs`, `bardecoder` | other QR detectors | quircs 0.10.3 (2025-10); bardecoder last updated 2023 | — | MIT |

All licences are already allowed by `deny.toml` (ISC is on the list).

## Options considered
1. **`rqrr` + `png` + `zune-jpeg`, used directly.** Smallest set that does the job; each decoder exposes limits we can set before any pixel buffer is allocated; no colour-management code. Cost: we write ~100 lines of glue (sniff format, decode to greyscale, hand to `rqrr`).
2. **`image` (png + jpeg features) + `rqrr`'s `image` feature.** Least glue code. Cost: pulls a colour-management crate and several helpers we do not need into a component that parses untrusted files, and its limit handling sits behind a general-purpose API.
3. **`rxing`.** Reads many barcode types. Cost: much larger attack surface for a feature that needs only QR.
4. **Decode in the webview (JavaScript/canvas + a JS QR library).** Rejected: the seed would enter JavaScript, violating ADR-0002.

## Decision
Option 1, in the `mfa-formats` crate (no I/O: the caller passes the file's bytes).

1. **Formats**: PNG and JPEG only, recognised by their first bytes, never by file name. Anything else is refused with a readable message.
2. **Limits, checked from the image header before any pixel buffer is allocated:**
   - file size ≤ 10 MiB;
   - width and height each ≤ 8192 and total pixels ≤ 25 million (a 5000 × 5000 image);
   - PNG decoder memory limit 128 MiB (so a 16-bit RGBA file near the pixel cap is refused rather than expanded);
   - at most 16 QR codes are returned from one image.
3. **Greyscale only**: decoded to one byte per pixel and passed to `rqrr`; no colour management.
4. **Results are plain text payloads** (strings). `parse_otpauth` (and later the migration parser) turns them into entries; the image decoder knows nothing about OTP.
5. **Errors are fixed, readable sentences** ("the image is too large", "no QR code found", "the file is not a PNG or JPEG image") and never contain file contents or decoded text.
6. **Tests with hostile input** (all required): empty input; random bytes; a PNG and a JPEG truncated at several points; a tiny PNG whose header declares a huge size; a header over the dimension limit; an image over the file-size limit; a valid image with no QR code; one with several QR codes; and valid PNG and JPEG images carrying a QR code (generated in the test with dev-only encoder crates).
7. **Time**: decoding is single-pass over a bounded number of pixels, so the pixel cap is also the time bound. The shell runs it off the UI thread; there is no separate timeout.

## Consequences
+ The seed stays in Rust from the image file to the stored entry.
+ Small, auditable dependency set; decoder limits are enforced by the decoders themselves.
- A few more crates in `Cargo.lock` (`g2p`, `lru` and the decoder crates); `cargo deny` and the CI audit cover them.
- Photos from 50-megapixel cameras are refused as "too large"; the user must use a smaller image or a screenshot. This can be raised later if it proves annoying.
- JPEG decoding quality varies; very blurry photos may not decode, and the app can only say "no QR code found".
