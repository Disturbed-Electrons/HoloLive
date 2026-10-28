use std::{
    fs::{self, File},
    io::{self, BufWriter},
    path::PathBuf,
};

use chrono::Local;
use hololive_core::{DisplayMode, Real};

use crate::levels;

pub(crate) fn save<T: Real>(
    values: &[T],
    width: usize,
    height: usize,
    display_mode: DisplayMode,
) -> io::Result<PathBuf> {
    let directory = PathBuf::from("exports");
    fs::create_dir_all(&directory)?;
    let timestamp = Local::now().format("%Y-%m-%d_%H-%M-%S%.3f");
    let kind = match display_mode {
        DisplayMode::Phase => "PH",
        DisplayMode::Amplitude => "AMP",
    };
    let png_path = directory.join(format!("HoloLive_{kind}_{timestamp}.png"));
    let writer = BufWriter::new(File::create(&png_path)?);
    let pixels = levels::minmax_normalize(values);
    let mut encoder = png::Encoder::new(writer, width as u32, height as u32);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::High);
    let mut png = encoder.write_header()?;
    png.write_image_data(&pixels)?;
    png.finish()?;
    Ok(png_path)
}
