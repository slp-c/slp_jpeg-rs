use std::ops;

// this thing is slow asf
pub fn inverse_dct(dst: &mut [i16; 64], src: &[i16; 64]) {
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = {
                let dst = ({
                    let mut sum: f32 = 0.0;
                    for v in 0..8 {
                        for u in 0..8 {
                            let (x, y, u, v) = (x as f32, y as f32, u as f32, v as f32);
                            let mut temp: f32 = src[v as usize * 8 + u as usize] as f32
                                * f32::cos((2.0 * x + 1.0) * u * std::f32::consts::PI / 16.0)
                                * f32::cos((2.0 * y + 1.0) * v * std::f32::consts::PI / 16.0);
                            if u as usize == 0 {
                                temp /= f32::sqrt(2.0);
                            }
                            if v as usize == 0 {
                                temp /= f32::sqrt(2.0);
                            }
                            sum += temp;
                        }
                    }
                    sum
                } / 4.0);
                let dst = unsafe { dst.to_int_unchecked::<i16>() } + 128;
                dst
            };
        }
    }
}

pub fn inverse_zigzag<T>(dst: &mut [T; 64], src: &[T; 64])
where
    T: Clone,
{
    #[rustfmt::skip]
    const TRANSLATION_TABLE: [u8; 64] = [
        0,  1,  5,  6,  14, 15, 27, 28,
        2,  4,  7,  13, 16, 26, 29, 42,
        3,  8,  12, 17, 25, 30, 41, 43,
        9,  11, 18, 24, 31, 40, 44, 53,
        10, 19, 23, 32, 39, 45, 52, 54,
        20, 22, 33, 38, 46, 51, 55, 60,
        21, 34, 37, 47, 50, 56, 59, 61,
        35, 36, 48, 49, 57, 58, 62, 63,
    ];
    for i in 0..64 {
        dst[i] = src[TRANSLATION_TABLE[i] as usize].clone();
    }
}

pub fn inverse_quant<T>(quant_table: &[T; 64], buf: &mut [T; 64])
where
    T: Clone + ops::MulAssign,
{
    for i in 0..64 {
        buf[i] *= quant_table[i].clone();
    }
}

pub fn convert_ycbcr2rgb(image: &mut [u8]) {
    // TODO: use fixed-point math
    for pixel in image.as_chunks_mut::<3>().0 {
        let y = pixel[0] as f32;
        let cb = pixel[1] as f32;
        let cr = pixel[2] as f32;

        pixel[0] = (y + 1.402 * (cr - 128.0)) as u8;
        pixel[1] = (y - 0.34414 * (cb - 128.0) - 0.71414 * (cr - 128.0)) as u8;
        pixel[2] = (y + 1.772 * (cb - 128.0)) as u8;
    }
}

#[allow(unused)]
pub fn zigzag<T>(dst: &mut [T; 64], src: &[T; 64])
where
    T: Clone,
{
    #[rustfmt::skip]
    const TRANSLATION_TABLE: [u8; 64] = [
        0,  1,  5,  6,  14, 15, 27, 28,
        2,  4,  7,  13, 16, 26, 29, 42,
        3,  8,  12, 17, 25, 30, 41, 43,
        9,  11, 18, 24, 31, 40, 44, 53,
        10, 19, 23, 32, 39, 45, 52, 54,
        20, 22, 33, 38, 46, 51, 55, 60,
        21, 34, 37, 47, 50, 56, 59, 61,
        35, 36, 48, 49, 57, 58, 62, 63,
    ];
    for i in 0..64 {
        dst[i] = src[TRANSLATION_TABLE[i] as usize].clone();
    }
}
#[allow(unused)]
pub fn quant<T>(quant_table: &[T; 64], buf: &mut [T; 64])
where
    T: Clone + ops::DivAssign,
{
    for i in 0..64 {
        buf[i] /= quant_table[i].clone();
    }
}
