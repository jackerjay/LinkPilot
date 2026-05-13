//! Re-pad the brand PNG to Apple's icon grid.
//!
//! Apple's macOS Big Sur+ icon guidelines: the artwork should occupy
//! ~824 of 1024 pixels (≈ 80.5%) centered on a transparent canvas. The
//! system applies its own rounded-rect mask + drop shadow on top, so a
//! source PNG whose artwork fills 100% of the canvas (and bakes its own
//! background in) ends up looking cramped and double-rounded in the Dock.
//!
//! This binary reads a square PNG, downscales it onto an N% interior of
//! a same-size transparent canvas, and writes the result. Run it once
//! after you update `docs/brand/icon.png`; commit the padded file in
//! place of the master.
//!
//! Usage:
//!   cargo run -p linkpilot-icon-padder --release -- INPUT OUTPUT [SCALE]
//!
//!   SCALE is the artwork-as-fraction-of-canvas, default 0.82.

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let Some(input) = args.next().map(PathBuf::from) else {
        eprintln!("usage: linkpilot-icon-padder INPUT OUTPUT [SCALE=0.82]");
        return ExitCode::FAILURE;
    };
    let Some(output) = args.next().map(PathBuf::from) else {
        eprintln!("usage: linkpilot-icon-padder INPUT OUTPUT [SCALE=0.82]");
        return ExitCode::FAILURE;
    };
    let scale: f32 = args
        .next()
        .map(|s| s.parse().unwrap_or(0.82))
        .unwrap_or(0.82);
    if !(0.1..=1.0).contains(&scale) {
        eprintln!("SCALE must be in (0.1, 1.0], got {scale}");
        return ExitCode::FAILURE;
    }

    let src = match image::open(&input) {
        Ok(i) => i.into_rgba8(),
        Err(e) => {
            eprintln!("could not read {}: {e}", input.display());
            return ExitCode::FAILURE;
        }
    };
    let (w, h) = (src.width(), src.height());
    if w != h {
        eprintln!(
            "input must be square (got {w}x{h}); macOS icons assume a 1:1 canvas",
        );
        return ExitCode::FAILURE;
    }

    // Downscale the artwork to `scale * canvas`, then paste onto a
    // transparent canvas of the original size.
    let inner = (w as f32 * scale).round() as u32;
    let resized = image::imageops::resize(&src, inner, inner, FilterType::Lanczos3);
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let offset = (w - inner) / 2;
    image::imageops::overlay(&mut canvas, &resized, offset as i64, offset as i64);

    if let Err(e) = canvas.save(&output) {
        eprintln!("could not write {}: {e}", output.display());
        return ExitCode::FAILURE;
    }

    println!(
        "wrote {} ({w}x{h}, artwork {inner}x{inner} centered at {offset},{offset})",
        output.display(),
    );
    ExitCode::SUCCESS
}
