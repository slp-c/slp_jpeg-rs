use std::time::Instant;

use slp_jpeg_rs::{Image, read::JpegDecoderError};

#[allow(unused)]
#[derive(Debug)]
enum Error {
    ImageRead(JpegDecoderError),
}

const INPUT_FILE: &'static str = "/home/rei/Projects/Rust/slp_jpeg-rs/input.jpg";

#[test]
fn main() -> Result<(), Error> {
    let start = Instant::now();

    let image: Image = Image::read_jpeg_from_file(INPUT_FILE).map_err(|e| Error::ImageRead(e))?;

    println!("total: {}", start.elapsed().as_secs_f64());

    let start = Instant::now();

    let jpeg_data = std::fs::read(INPUT_FILE).unwrap();
    let _ = turbojpeg::decompress(&jpeg_data, turbojpeg::PixelFormat::RGB).unwrap();

    println!("turbojpeg total: {}", start.elapsed().as_secs_f64());

    let image = turbojpeg::Image {
        pixels: &image.pixels[..],
        width: image.width as usize,
        pitch: (image.width as usize * image.channels as usize * image.bit_depth as usize)
            .div_ceil(8),
        height: image.height as usize,
        format: turbojpeg::PixelFormat::RGB,
    };
    let jpeg_data = turbojpeg::compress(image, 100, turbojpeg::Subsamp::Sub4x1).unwrap();
    std::fs::write("output.jpg", &jpeg_data).unwrap();

    Ok(())
}
