//! Shape the brand PNG into a macOS-style icon: artwork at ~82% of the
//! canvas, centered, with a continuous-curvature squircle (superellipse,
//! exponent 5) as the visible outer edge. Pixels outside the squircle
//! get alpha = 0 so the Dock / Launchpad render the proper "rounded
//! square" silhouette that every native macOS app has.
//!
//! Apple's actual Big Sur+ icon shape is a *continuous-curvature*
//! squircle, not a regular rounded rectangle. The superellipse equation
//!     |u|^n + |v|^n <= 1
//! with n ≈ 5 is the standard close approximation and matches their
//! Sketch / Figma template visually at icon sizes.
//!
//! Usage:
//!   cargo run -p linkpilot-icon-padder --release -- INPUT OUTPUT \
//!     [--scale 0.82] [--exp 5.0]
//!
//!   --scale   artwork width as fraction of canvas (default 0.82)
//!   --exp     superellipse exponent (default 5.0; bigger → squarer)

use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

use image::imageops::FilterType;
use image::{ImageBuffer, Rgba, RgbaImage};

#[derive(Debug)]
struct Args {
    input: PathBuf,
    output: PathBuf,
    scale: f32,
    exp: f32,
}

fn parse_args() -> Option<Args> {
    let mut iter = env::args().skip(1);
    let input = iter.next()?.into();
    let output = iter.next()?.into();
    let mut scale = 0.82_f32;
    let mut exp = 5.0_f32;
    while let Some(flag) = iter.next() {
        let val = iter.next()?;
        match flag.as_str() {
            "--scale" => scale = val.parse().ok()?,
            "--exp" => exp = val.parse().ok()?,
            _ => return None,
        }
    }
    Some(Args { input, output, scale, exp })
}

fn main() -> ExitCode {
    let Some(args) = parse_args() else {
        eprintln!(
            "usage: linkpilot-icon-padder INPUT OUTPUT [--scale 0.82] [--exp 5.0]",
        );
        return ExitCode::FAILURE;
    };
    if !(0.1..=1.0).contains(&args.scale) {
        eprintln!("--scale must be in (0.1, 1.0]");
        return ExitCode::FAILURE;
    }

    let src = match image::open(&args.input) {
        Ok(i) => i.into_rgba8(),
        Err(e) => {
            eprintln!("could not read {}: {e}", args.input.display());
            return ExitCode::FAILURE;
        }
    };
    let (w, h) = (src.width(), src.height());
    if w != h {
        eprintln!("input must be square; got {w}x{h}");
        return ExitCode::FAILURE;
    }

    // Step 1: downscale the source to scale × canvas, centered on a
    // transparent canvas of the original size.
    let inner = (w as f32 * args.scale).round() as u32;
    let resized = image::imageops::resize(&src, inner, inner, FilterType::Lanczos3);
    let mut canvas: RgbaImage = ImageBuffer::from_pixel(w, h, Rgba([0, 0, 0, 0]));
    let offset = ((w - inner) / 2) as i64;
    image::imageops::overlay(&mut canvas, &resized, offset, offset);

    // Step 2: apply a superellipse (squircle) mask. The squircle's
    // outer bounding box exactly matches the artwork's box, so the
    // mask trims the artwork's existing corners into the macOS shape
    // and leaves the transparent margin outside untouched.
    let cx = (w as f32 - 1.0) / 2.0;
    let cy = (h as f32 - 1.0) / 2.0;
    let r = inner as f32 / 2.0;
    let n = args.exp;
    let edge_aa = 1.5_f32; // pixels of anti-alias falloff at the edge

    for y in 0..h {
        for x in 0..w {
            let u = (x as f32 - cx) / r;
            let v = (y as f32 - cy) / r;
            // d = (|u|^n + |v|^n)^(1/n); d == 1 → on the squircle edge.
            let d = (u.abs().powf(n) + v.abs().powf(n)).powf(1.0 / n);
            if d <= 1.0 {
                continue; // fully inside
            }
            // Linear fade from 1 (just outside) to 0 (edge_aa pixels out).
            // The conversion from d to "pixels past the edge" is roughly
            //   (d - 1) * r  along the gradient direction.
            let pixels_past = (d - 1.0) * r;
            let alpha_scale = (1.0 - pixels_past / edge_aa).clamp(0.0, 1.0);
            let p = canvas.get_pixel_mut(x, y);
            p.0[3] = (p.0[3] as f32 * alpha_scale).round() as u8;
        }
    }

    if let Err(e) = canvas.save(&args.output) {
        eprintln!("could not write {}: {e}", args.output.display());
        return ExitCode::FAILURE;
    }

    println!(
        "wrote {} ({w}x{h}, artwork {inner}x{inner} clipped to superellipse n={n})",
        args.output.display(),
    );
    ExitCode::SUCCESS
}
