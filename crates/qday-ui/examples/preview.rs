//! Renders every screen in both themes to PNG: `cargo run -p qday-ui --example preview <folder>`.

use std::fs::{File, create_dir_all};
use std::io::BufWriter;
use std::path::Path;

use qday_ui::fb::{Framebuffer, HEIGHT, WIDTH};
use qday_ui::gallery::Samples;
use qday_ui::theme::{DARK, LIGHT, Theme};

fn rgb888(pixel: u16) -> [u8; 3] {
    let red = u32::from(pixel >> 11) & 0x1F;
    let green = u32::from(pixel >> 5) & 0x3F;
    let blue = u32::from(pixel) & 0x1F;
    let expand = |value: u32, top: u32| u8::try_from(value * 255 / top).unwrap_or(255);
    [expand(red, 31), expand(green, 63), expand(blue, 31)]
}

fn save(fb: &Framebuffer, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let width = u32::try_from(WIDTH)?;
    let height = u32::try_from(HEIGHT)?;
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path)?), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    let mut bytes = Vec::new();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            bytes.extend(rgb888(fb.pixel(x, y)));
        }
    }
    writer.write_image_data(&bytes)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let folder = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "previews".to_string());
    let folder = Path::new(&folder);
    create_dir_all(folder)?;

    let samples = Samples::new();
    let themes: [(&str, &Theme); 2] = [("light", &LIGHT), ("dark", &DARK)];
    for (theme_name, theme) in themes {
        for (name, screen) in samples.screens() {
            let mut fb = Framebuffer::new();
            screen.draw(&mut fb, theme);
            save(&fb, &folder.join(format!("{name}-{theme_name}.png")))?;
        }
    }
    println!(
        "{} screens in {}",
        samples.screens().len() * 2,
        folder.display()
    );
    Ok(())
}
