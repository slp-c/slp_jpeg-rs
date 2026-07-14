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
                let dst = unsafe { dst.to_int_unchecked::<i16>() };
                dst
            };
        }
    }
}
pub fn dct(dst: &mut [i16; 64], src: &[i16; 64]) {
    for y in 0..8 {
        for x in 0..8 {
            dst[y * 8 + x] = {
                let mut dst = ({
                    let mut sum: f32 = 0.0;
                    for v in 0..8 {
                        for u in 0..8 {
                            let (x, y, u, v) = (x as f32, y as f32, u as f32, v as f32);
                            sum += src[v as usize * 8 + u as usize] as f32
                                * f32::cos((2.0 * u + 1.0) * x * std::f32::consts::PI / 16.0)
                                * f32::cos((2.0 * v + 1.0) * y * std::f32::consts::PI / 16.0);
                        }
                    }
                    sum
                } / 4.0);
                if x == 0 {
                    dst /= f32::sqrt(2.0);
                }
                if y == 0 {
                    dst /= f32::sqrt(2.0);
                }
                let dst = unsafe { dst.to_int_unchecked::<i16>() };
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

pub fn inverse_quant<T>(quant_table: &[T; 64], buf: &mut [T; 64])
where
    T: Clone + std::ops::MulAssign,
{
    for i in 0..64 {
        buf[i] *= quant_table[i].clone();
    }
}
pub fn quant<T>(quant_table: &[T; 64], buf: &mut [T; 64])
where
    T: Clone + std::ops::DivAssign,
{
    for i in 0..64 {
        buf[i] /= quant_table[i].clone();
    }
}

pub fn convert_ycbcr2rgb(image: &mut [u8]) {
    type T = i32;
    const SHIFT: usize = size_of::<T>() * 8 / 2;
    const A: [[T; 3]; 3] = {
        #[rustfmt::skip]
        const A_FLOAT: [[f64; 3]; 3] = [
            [1.0, 0.0, 1.402],
            [1.0, -0.34414, -0.71414],
            [1.0, 1.772, 0.0]
        ];

        let mut x: [[T; 3]; 3] = [[0; 3]; 3];
        let mut i = 0;
        while i < 3 {
            let mut j = 0;
            while j < 3 {
                x[i][j] = (A_FLOAT[i][j] * (1u64 << SHIFT) as f64) as T;
                j += 1;
            }
            i += 1;
        }

        x
    };

    for pixel in image.as_chunks_mut::<3>().0 {
        let y = pixel[0] as T;
        let cb = pixel[1] as T - 128;
        let cr = pixel[2] as T - 128;

        #[rustfmt::skip]
        {
            pixel[0] = ((A[0][0] * y + A[0][1] * cb + A[0][2] * cr) >> SHIFT).clamp(0, 255) as u8;
            pixel[1] = ((A[1][0] * y + A[1][1] * cb + A[1][2] * cr) >> SHIFT).clamp(0, 255) as u8;
            pixel[2] = ((A[2][0] * y + A[2][1] * cb + A[2][2] * cr) >> SHIFT).clamp(0, 255) as u8;
        };
    }
}
pub fn extract_ycbcr2rgb(image: &[u8], r: &mut [u8], g: &mut [u8], b: &mut [u8]) {
    /*
    assume r, g, b have the same len and panic if not
    */
    type T = i32;
    const SHIFT: usize = size_of::<T>() * 8 / 2;
    const A: [[T; 3]; 3] = {
        #[rustfmt::skip]
        const A_FLOAT: [[f64; 3]; 3] = [
            [1.0, 0.0, 1.402],
            [1.0, -0.34414, -0.71414],
            [1.0, 1.772, 0.0]
        ];

        let mut x: [[T; 3]; 3] = [[0; 3]; 3];
        let mut i = 0;
        while i < 3 {
            let mut j = 0;
            while j < 3 {
                x[i][j] = (A_FLOAT[i][j] * (1u64 << SHIFT) as f64) as T;
                j += 1;
            }
            i += 1;
        }

        x
    };

    let mut r = r.iter_mut();
    let mut g = g.iter_mut();
    let mut b = b.iter_mut();

    for pixel in image.as_chunks::<3>().0 {
        let y = pixel[0] as T;
        let cb = pixel[1] as T - 128;
        let cr = pixel[2] as T - 128;

        let r = r.next().unwrap();
        let g = g.next().unwrap();
        let b = b.next().unwrap();

        *r = ((A[0][0] * y + A[0][1] * cb + A[0][2] * cr) >> SHIFT).clamp(0, 255) as u8;
        *g = ((A[1][0] * y + A[1][1] * cb + A[1][2] * cr) >> SHIFT).clamp(0, 255) as u8;
        *b = ((A[2][0] * y + A[2][1] * cb + A[2][2] * cr) >> SHIFT).clamp(0, 255) as u8;
    }
}
pub fn convert_rgb2ycbcr(image: &mut [u8]) {
    type T = i32;
    const SHIFT: usize = size_of::<T>() * 8 / 2;
    const A: [[T; 3]; 3] = {
        #[rustfmt::skip]
        const A_FLOAT: [[f64; 3]; 3] = [
            [0.299, 0.587, 0.114],
            [-0.168736, -0.331264, 0.5],
            [0.5, -0.418688, -0.081312]
        ];

        let mut x: [[T; 3]; 3] = [[0; 3]; 3];
        let mut i = 0;
        while i < 3 {
            let mut j = 0;
            while j < 3 {
                x[i][j] = (A_FLOAT[i][j] * (1u64 << SHIFT) as f64) as T;
                j += 1;
            }
            i += 1;
        }

        x
    };

    for pixel in image.as_chunks_mut::<3>().0 {
        let r = pixel[0] as T;
        let g = pixel[1] as T;
        let b = pixel[2] as T;

        #[rustfmt::skip]
        {
            pixel[0] = (((A[0][0] * r + A[0][1] * g + A[0][2] * b) >> SHIFT)).clamp(0, 255) as u8;
            pixel[1] = (((A[1][0] * r + A[1][1] * g + A[1][2] * b) >> SHIFT) + 128).clamp(0, 255) as u8;
            pixel[2] = (((A[2][0] * r + A[2][1] * g + A[2][2] * b) >> SHIFT) + 128).clamp(0, 255) as u8;
        };
    }
}
pub fn extract_rgb2ycbcr(image: &[u8], y: &mut [u8], cb: &mut [u8], cr: &mut [u8]) {
    /*
    assume y, cb, cr have the same len and panic if not
    */
    type T = i32;
    const SHIFT: usize = size_of::<T>() * 8 / 2;
    const A: [[T; 3]; 3] = {
        #[rustfmt::skip]
        const A_FLOAT: [[f64; 3]; 3] = [
            [0.299, 0.587, 0.114],
            [-0.168736, -0.331264, 0.5],
            [0.5, -0.418688, -0.081312]
        ];

        let mut x: [[T; 3]; 3] = [[0; 3]; 3];
        let mut i = 0;
        while i < 3 {
            let mut j = 0;
            while j < 3 {
                x[i][j] = (A_FLOAT[i][j] * (1u64 << SHIFT) as f64) as T;
                j += 1;
            }
            i += 1;
        }

        x
    };

    let mut y = y.iter_mut();
    let mut cb = cb.iter_mut();
    let mut cr = cr.iter_mut();

    for pixel in image.as_chunks::<3>().0 {
        let r = pixel[0] as T;
        let g = pixel[1] as T;
        let b = pixel[2] as T;

        let y = y.next().unwrap();
        let cb = cb.next().unwrap();
        let cr = cr.next().unwrap();

        *y = ((A[0][0] * r + A[0][1] * g + A[0][2] * b) >> SHIFT).clamp(0, 255) as u8;
        *cb = (((A[1][0] * r + A[1][1] * g + A[1][2] * b) >> SHIFT) + 128).clamp(0, 255) as u8;
        *cr = (((A[2][0] * r + A[2][1] * g + A[2][2] * b) >> SHIFT) + 128).clamp(0, 255) as u8;
    }
}

pub fn subsampling(src: &[u8], i_factor: usize, j_factor: usize, width: usize) -> Vec<u8> {
    if i_factor == 0 || j_factor == 0 {
        return Vec::new();
    } else if i_factor == 1 && j_factor == 1 {
        return Vec::from(src);
    }

    let height = src.len() / width;

    let new_width = width.div_ceil(j_factor);
    let new_height = height.div_ceil(i_factor);

    let mut dst: Vec<u8> = vec![0; new_width * new_height];
    let block_len = i_factor * j_factor;

    for y in 0..new_height {
        for x in 0..new_width {
            let base_src_index = y * i_factor * width + x * j_factor;

            let mut block_sum: usize = 0;
            for i in 0..i_factor {
                for j in 0..j_factor {
                    block_sum += src[base_src_index + i * width + j] as usize;
                }
            }

            dst[y * new_width + x] = (block_sum / block_len) as u8;
        }
    }

    return dst;
}
