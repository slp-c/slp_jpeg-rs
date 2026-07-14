use crate::read::{JpegDecoder, JpegDecoderError};

pub mod algorithm;
pub mod read;
pub mod write;

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
    pub fn read_jpeg_from_file(path: &str) -> Result<Image, JpegDecoderError> {
        let jpeg_file = std::fs::read(path)?;
        let mut reader = std::io::Cursor::new(jpeg_file.as_slice());

        let mut image = Image::default();
        let mut decoder = JpegDecoder::init(&mut reader, &mut image)?;
        // after JpegDecoder::init ALL pub fn in the decoder is available to call!

        image.pixels = vec![
                0; // the formula is height * bytes_per_row. Some might refer bytes_per_row as row_strides or pitch
                image.height as usize
                    * (image.width as usize * image.channels as usize * image.bit_depth as usize)
                        .div_ceil(8) // this is redundant for baseline jpeg but it's just a universal way to do it so we keep it anyway
            ];

        /*
        decode_next_block will do Huffman + RLE decoding, it will return Err() on error and Ok() on success
        if return Ok() it'll be Ok(Some(block)) if there's still some block to decode
        and return Ok(None) if there's no more to decode
        it was design to use with the while let as you see below
        */

        let mut temp: [i16; 64] = [0; 64];
        while let Some(mut block) = decoder.decode_next_block(&mut reader)? {
            algorithm::inverse_quant(decoder.get_quant_table(block.component), &mut block.data);
            algorithm::inverse_zigzag(&mut temp, &block.data);
            algorithm::inverse_dct(&mut block.data, &temp);
            block.data.iter_mut().for_each(|x| *x += 128);
            decoder.write_block(&mut image, &block);
            // developers should be able to define write_block by themselves if they want
            // See struct Block in this same file
        }
        algorithm::convert_ycbcr2rgb(&mut image.pixels);

        Ok(image)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Block<T>
where
    T: Clone + Copy + From<u8>,
{
    pub data: [T; 64],
    pub mcu: usize,
    pub component: u8, // 0, 1, 2
    pub v: u8,
    pub h: u8,
    /*
    mcu is the Minimum Coded Units
    We use mcu as the block position for our decoder.

    Size of an mcu block is chosen to be the same size as
    the prime_component (decoder.get_prime_component()).

    The prime_component is the one decoder choose so other component will scale into it

    component will tell you each component this block have.
    we define:
    component: 0 -> Y
    compoennt: 1 -> Cb
    component: 2 -> Cr
    this allow we to use them as index

    v and h
    For example component 0 (Y)
    This component have
    - vertical sampling factor = 2
    - horizontal sampling factor = 2
    So a full component Y is a 2x2 block!
    1 component Y is 4 blocks follow this order
    Using a[v][h] we got
    [a00, a01, a10, a11],
    Each component like this will be on the image as a whole 16x16 block! (assume Y is the prime_component)

    So in order to write we need to know, which mcu are we writting,
    then which block in that mcu we're writting
    For component that have less/equal/greater sampling factor
    than/as/than prime component we sacle/keep/scale the block size up//down
    */
}
