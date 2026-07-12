use std::{io, time::Instant};

use slp_jpeg_rs::Image;

use turbojpeg::{PixelFormat, decompress};

#[allow(dead_code)]
#[derive(Debug)]
enum Error {
    FileOpen(io::Error),
    ImageRead,
    ImageSave(image::ImageError),
}

#[test]
fn main() -> Result<(), Error> {
    let start = Instant::now();
    let image: Image = Image::read_jpeg_from_file("/home/rei/Projects/Rust/slp_jpeg-rs/input.jpg")
        .map_err(|_| Error::ImageRead)?;
    println!("total: {}", start.elapsed().as_secs_f64());

    let start = Instant::now();
    let jpeg_data = std::fs::read("/home/rei/Projects/Rust/slp_jpeg-rs/input.jpg")
        .map_err(|v| Error::FileOpen(v))?;
    let _image = decompress(&jpeg_data, PixelFormat::RGB).map_err(|_| Error::ImageRead)?;
    println!("turbojpeg total: {}", start.elapsed().as_secs_f64());

    image::save_buffer(
        "output.png",
        &image.pixels,
        image.width as u32,
        image.height as u32,
        image::ColorType::Rgb8,
    )
    .map_err(|v| Error::ImageSave(v))?;

    Ok(())
}
