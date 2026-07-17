use std::time::Instant;

use slp_jpeg_rs::{Image, read::JpegDecoderError, read_jpeg_from_file_fast};

const INPUT_FILE: &'static str = "/home/rei/Projects/Rust/slp_jpeg-rs/input_.jpg";

#[test]
fn main() -> Result<(), Error> {
    let start = Instant::now();

    let image: Image = read_jpeg_from_file_fast(INPUT_FILE)?;

    println!("slp read: {} ms", start.elapsed().as_millis());

    {
        let start = Instant::now();

        let jpeg_data = std::fs::read(INPUT_FILE).unwrap();
        let img = turbojpeg::decompress(&jpeg_data, turbojpeg::PixelFormat::RGB).unwrap();

        println!("turbojpeg read: {} ms", start.elapsed().as_millis());

        if image.pixels.len() != img.pixels.len() {
            panic!(
                "Output len is {}, but turbojpeg is {}",
                image.pixels.len(),
                img.pixels.len()
            );
        }

        let mut mismatch_counter = 0;
        for (a, b) in img.pixels.iter().zip(image.pixels.iter()) {
            let a = *a as i16;
            let b = *b as i16;
            let error = a.abs_diff(b);
            if error > 5 {
                mismatch_counter += 1;
                // error less than 5% of the image
                if mismatch_counter >= image.pixels.len() / 20 {
                    panic!(
                        "Output mismatch. Precision: {}",
                        (mismatch_counter * 100) as f64 / image.pixels.len() as f64
                    );
                }
            }
        }

        println!(
            "Precision: {:.2}%",
            (mismatch_counter * 100) as f64 / image.pixels.len() as f64
        );
    }

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

#[allow(unused)]
#[derive(Debug)]
enum Error {
    ImageRead(JpegDecoderError),
}

impl From<JpegDecoderError> for Error {
    fn from(value: JpegDecoderError) -> Self {
        Self::ImageRead(value)
    }
}
