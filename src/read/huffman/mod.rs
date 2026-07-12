mod bit_reader;

use std::io;

use crate::{
    EOI, SOS,
    read::{
        huffman::bit_reader::{BitReader, BitReaderError},
        parser::HuffmanSymbol,
    },
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HuffmanDecoder {
    reader: BitReader,
    previous_dc: [i16; 3],
}

impl HuffmanDecoder {
    pub fn new<R: io::Read + io::Seek>(reader: &mut R) -> Result<Self, HuffmanDecoderError> {
        Ok(Self {
            reader: BitReader::new(reader)?,
            previous_dc: [0; 3],
        })
    }

    // read_huffman will not consume any bits
    fn read_huffman(
        &self,
        huffman_table: &[HuffmanSymbol; 65536],
    ) -> Result<HuffmanSymbol, HuffmanDecoderError> {
        let mut code: u16 = 0;
        let mut code_len: u8 = 0;

        while code_len < 16 {
            code <<= 1;
            code |= ((self.reader.buf << code_len) & 0x8000) >> 15;
            code_len += 1;

            if code_len == huffman_table[code as usize].code_len {
                return Ok(huffman_table[code as usize]);
            }
        }

        Err(HuffmanDecoderError::UnknownSymbol)
    }

    pub fn decode<T, R>(
        &mut self,
        reader: &mut R,
        huffman_table: [&[HuffmanSymbol; 65536]; 2], // [DC, AC]
        component: u8, // which component are we decoding? this is required for previous_dc to work
        dest: &mut [T],
    ) -> Result<(), HuffmanDecoderError>
    where
        T: From<i16>,
        R: io::Read + io::Seek,
    {
        let symbol: HuffmanSymbol = self.read_huffman(huffman_table[0])?;
        self.reader.read_bits(reader, symbol.code_len)?;

        let t = symbol.symbol;
        if t >= 16 {
            return Err(HuffmanDecoderError::StreamError);
        }

        let dc_diff: i16;
        {
            if t == 0 {
                dc_diff = 0;
            } else {
                let mut v = self.reader.read_bits(reader, t)? as i32;
                let mut vt = 1 << (t - 1);
                if v < vt {
                    vt = (-1i32 << t) + 1;
                    v += vt;
                }
                dc_diff = v as i16;
            }
        }

        let dc: i16 = self.previous_dc[component as usize].wrapping_add(dc_diff);
        self.previous_dc[component as usize] = dc;
        dest[0] = dc.into();

        // decode block[1..64] with AC table
        let mut i: usize = 1;
        while i < 64 {
            let symbol: HuffmanSymbol = self.read_huffman(huffman_table[1])?;

            match self.reader.read_bits(reader, symbol.code_len) {
                Err(BitReaderError::Marker(EOI)) | Err(BitReaderError::Marker(SOS)) => break,
                v => v,
            }?;

            let s = (symbol.symbol >> 0) & 0x0F; // size
            let r = (symbol.symbol >> 4) & 0x0F; // run

            if s == 0 {
                if r == 15 {
                    i += 16;
                    continue;
                } else {
                    break;
                }
            }

            i += r as usize;
            if i >= 64 {
                break;
            }

            dest[i] = {
                let mut v: i32 = self.reader.read_bits(reader, s)? as i32;

                let mut vt: i32 = 1 << (s - 1);
                if v < vt {
                    vt = (-1i32 << s) + 1;
                    v += vt;
                }

                (v as i16).into()
            };

            i += 1;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HuffmanDecoderError {
    ReadFail(io::ErrorKind),
    UnknownSymbol,
    Marker(u8),
    StreamError,
}

impl From<BitReaderError> for HuffmanDecoderError {
    fn from(value: BitReaderError) -> Self {
        match value {
            BitReaderError::ReadFail(e) => HuffmanDecoderError::ReadFail(e),
            BitReaderError::InputError => HuffmanDecoderError::StreamError,
            BitReaderError::Marker(v) => HuffmanDecoderError::Marker(v),
        }
    }
}

impl From<HuffmanDecoderError> for () {
    fn from(_: HuffmanDecoderError) -> Self {
        ()
    }
}
