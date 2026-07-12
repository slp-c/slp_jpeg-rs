use crate::{algorithm::convert_ycbcr2rgb, read::JpegDecoder};

pub(crate) mod algorithm;
pub mod read;

#[derive(Debug, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Image {
    pub pixels: Vec<u8>,
    pub bit_depth: u8,
    pub height: u16,
    pub width: u16,
    pub channels: u8,
}

pub(crate) const DQT: u8 = 0xDB;
pub(crate) const SOI: u8 = 0xD8;
pub(crate) const SOF0: u8 = 0xC0;
pub(crate) const DHT: u8 = 0xC4;
pub(crate) const SOS: u8 = 0xDA;
pub(crate) const EOI: u8 = 0xD9;

impl Image {
    pub fn read_jpeg_from_file(path: &str) -> Result<Image, ()> {
        let jpeg_file = std::fs::read(path).map_err(|_| ())?;
        let mut reader = std::io::Cursor::new(jpeg_file.as_slice());

        let mut image = Image::default();
        let mut decoder = JpegDecoder::init(&mut reader, &mut image)?;

        image.pixels = vec![
                0; // the formula is height * bytes_per_row. Some might refer bytes_per_row as row_strides
                image.height as usize
                    * (image.width as usize * image.channels as usize * image.bit_depth as usize)
                        .div_ceil(8) // this is redundant but it's just a universal way to do it so we keep it anyway
            ];

        let mut temp: [i16; 64] = [0; 64];
        while let Some(mut block) = decoder.decode_next_block(&mut reader)? {
            algorithm::inverse_quant(decoder.get_quant_table(block.component), &mut block.data);
            algorithm::inverse_zigzag(&mut temp, &block.data);
            algorithm::inverse_dct(&mut block.data, &temp);
            decoder.write_block(&block, &mut image);
        }

        convert_ycbcr2rgb(&mut image.pixels);

        Ok(image)
    }
}
