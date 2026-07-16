use std::{
    collections::VecDeque,
    sync::{Arc, mpsc},
    thread,
    time::Instant,
};

use crate::read::{JpegDecoder, JpegDecoderError, parser::ComponentTable};

pub mod algorithm;
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

pub fn read_jpeg_from_file(path: &str) -> Result<Image, JpegDecoderError> {
    let jpeg_file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(jpeg_file);

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
    decoder.decode_next_block will do Huffman + RLE decoding, it will return Err() on error and Ok() on success
    if return Ok() it'll be Ok(Some(block)) if there's still some block to decode
    and return Ok(None) if there's no more to decode
    it was design to use with the while let as you see below
    */
    let mut t = 0f32;

    let mut temp: [i16; 64] = [0; 64];
    while let Some(mut block) = decoder.decode_next_block(&mut reader)? {
        algorithm::inverse_quant(decoder.get_quant_table(block.component), &mut block.data);
        algorithm::inverse_zigzag(&mut temp, &block.data);
        algorithm::inverse_discrete_cosine_transform(&mut block.data, &temp);
        block.data.iter_mut().for_each(|x| *x += 128);
        let start = Instant::now();
        decoder.write_block(&mut image, &block);
        t += start.elapsed().as_secs_f32();
        // Developers should be able to define write_block by themselves if they want
        // See struct Block in this same file
        // This current write_block can write no conflicts
    }

    if image.channels == 3 {
        algorithm::convert_ycbcr2rgb(&mut image.pixels);
    }

    println!("{t}");

    Ok(image)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Block {
    pub data: [i16; 64],
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
    But of course you can choose otherwise,
    developers can define there own write_block function.
    You can read our write_block function in read.rs to see
    how we calculate the block position to write.

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
    than/as/than prime component we sacle/keep/scale the block size up//down.
    */
}

#[derive(Debug, Clone)]
struct ProcessBlockArgs {
    pixels: *mut u8,
    height: u16,
    width: u16,
    channels: u8,

    component_table: [ComponentTable; 3],
    quant_table: [Vec<i16>; 4],
    prime_component: usize,
}
unsafe impl Send for ProcessBlockArgs {}
unsafe impl Sync for ProcessBlockArgs {}

pub fn read_jpeg_from_file_fast(path: &str) -> Result<Image, JpegDecoderError> {
    let jpeg_file = std::fs::read(path)?;
    let mut reader = std::io::Cursor::new(jpeg_file.as_slice());

    let mut image = Image::default();
    let mut decoder = JpegDecoder::init(&mut reader, &mut image)?;

    image.pixels =
        vec![
            0;
            image.height as usize
                * (image.width as usize * image.channels as usize * image.bit_depth as usize)
                    .div_ceil(8)
        ];

    {
        let args = Arc::new(ProcessBlockArgs {
            pixels: image.pixels.as_mut_ptr(),
            height: image.height,
            width: image.width,
            channels: image.channels,

            component_table: decoder.clone_component_table(),
            quant_table: decoder.clone_quant_table(),
            prime_component: decoder.get_prime_component() as usize,
        });

        let blocks_per_row = (image.width as usize).div_ceil(
            8 * decoder
                .get_component_table(decoder.get_prime_component())
                .horizontal_sampling_factor as usize,
        );

        let mut senders = Vec::new();
        let mut handler = VecDeque::new();

        let worker_count: usize = match thread::available_parallelism() {
            Err(_) => 1,
            Ok(v) => v.get(),
        };

        for _ in 0..worker_count {
            let (tx, rx) = mpsc::channel::<VecDeque<Block>>();
            senders.push(tx);

            let args = args.clone();
            handler.push_back(thread::spawn(move || {
                let (ty, ry) = mpsc::channel::<Block>();

                let handler = {
                    let args = args.clone();
                    thread::spawn(move || {
                        while let Ok(block) = ry.recv() {
                            write_block(&args, &block);
                        }
                    })
                };

                let mut temp: [i16; 64] = [0; 64];
                while let Ok(mut blocks) = rx.recv() {
                    while let Some(mut block) = blocks.pop_front() {
                        let quant_table: &[i16; 64] = &args.quant_table
                            [args.component_table[block.component as usize].quant_id as usize]
                            [0..64]
                            .try_into()
                            .unwrap();

                        algorithm::inverse_quant(quant_table, &mut block.data);
                        algorithm::inverse_zigzag(&mut temp, &block.data);
                        algorithm::inverse_discrete_cosine_transform(&mut block.data, &temp);
                        block.data.iter_mut().for_each(|x| *x += 128);

                        ty.send(block).unwrap();
                    }
                }

                drop(ty);
                handler.join().unwrap();
            }));
        }

        let mut cur_worker = 0;
        let mut blocks: VecDeque<Block> = VecDeque::new();

        while let Some(block) = decoder.decode_next_block(&mut reader)? {
            if block.mcu % blocks_per_row == 0 {
                if senders[cur_worker].send(blocks.clone()).is_err() {
                    return Err(JpegDecoderError::StateError);
                }
                blocks.clear();
                cur_worker += 1;
                cur_worker %= worker_count;
            }

            blocks.push_back(block);
        }

        if !blocks.is_empty() {
            if senders[cur_worker].send(blocks.clone()).is_err() {
                return Err(JpegDecoderError::StateError);
            }
            blocks.clear();
        }

        drop(senders);
        while let Some(v) = handler.pop_front() {
            if v.join().is_err() {
                return Err(JpegDecoderError::StateError);
            }
        }
    }

    if image.channels == 3 {
        let pixels = image.pixels.len() / 3;
        let mut buf = &mut image.pixels[..];

        let p = match thread::available_parallelism() {
            Err(_) => 1,
            Ok(v) => v.get(),
        };
        let pix_per_thr = pixels / p;

        thread::scope(|s| {
            for _ in 0..p {
                let (cur, left) = buf.split_at_mut(pix_per_thr * 3);
                buf = left;

                s.spawn(|| algorithm::convert_ycbcr2rgb(cur));
            }
            algorithm::convert_ycbcr2rgb(buf);
        });
    }

    Ok(image)
}

#[inline(always)]
fn write_block(arg: &ProcessBlockArgs, block: &Block) {
    let component = block.component as usize;
    let p = block.mcu as usize;
    let bpp: usize = arg.channels as usize;
    let bpr: usize = arg.width as usize * bpp;

    let i_factor = arg.component_table[component].i_factor;
    let j_factor = arg.component_table[component].j_factor;

    let block_height =
        8 * arg.component_table[arg.prime_component].vertical_sampling_factor as usize;
    let block_width =
        8 * arg.component_table[arg.prime_component].horizontal_sampling_factor as usize;

    let blocks_per_row = (arg.width as usize).div_ceil(block_width);

    let row = (p / blocks_per_row) * block_height + (block.v as usize) * 8 * i_factor;
    let col = (p % blocks_per_row) * block_width + (block.h as usize) * 8 * j_factor;

    let mut i: usize = 0;
    while i < 8 * i_factor && row + i < arg.height as usize {
        let mut j: usize = 0;
        while j < 8 * j_factor && col + j < arg.width as usize {
            let scaled_i = i / i_factor;
            let scaled_j = j / j_factor;

            let block_index = scaled_i * 8 + scaled_j;
            let pixel_index = (row + i) * bpr + (col + j) * bpp + component;

            unsafe {
                arg.pixels
                    .add(pixel_index)
                    .write(block.data[block_index].clamp(0, 0xFF) as u8);
            };

            j += 1;
        }
        i += 1;
    }
}
