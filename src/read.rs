use std::{cmp, io};

use crate::{
    Block, EOI, Image, SOS,
    read::{
        huffman::{HuffmanDecoder, HuffmanDecoderError},
        parser::{ComponentTable, JpegParser, ParserError},
    },
};

mod huffman;
mod parser;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JpegDecoder {
    parser: JpegParser,
    huffman: HuffmanDecoder,
    state: JpegDecoderState,
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct JpegDecoderState {
    mcu: usize,

    max_comp: u8,
    comp: u8,
    min_comp: u8,

    h: u8,
    v: u8,

    strm_end: bool,
}

impl JpegDecoder {
    pub fn init<R: io::Read + io::Seek>(
        reader: &mut R,
        image: &mut Image,
    ) -> Result<Self, JpegDecoderError> {
        let mut s = Self {
            parser: JpegParser::default(),
            huffman: HuffmanDecoder::default(),
            state: JpegDecoderState::default(),
        };
        s.parse(reader, &mut Some(image))?
            .ok_or(JpegDecoderError::EarlyEOI)?;
        Ok(s)
    }

    pub fn decode_next_block<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<Option<Block<i16>>, JpegDecoderError> {
        /*
        this function will return Err() on actuall Error
        return Ok(None) when there's no other block to decode next
        return Ok(Some(block)) when there's still some blocks to decode

        this function is stateful
        */
        if self.state.strm_end {
            return Ok(None);
        }

        let (mcu, component, v, h) = self.update_state(reader)?;

        let mut block = Block::<i16> {
            data: [0; 64],
            mcu,
            component,
            v,
            h,
        };

        self.huffman.decode(
            reader,
            self.parser.get_huffman_table(component),
            component,
            &mut block.data,
        )?;

        Ok(Some(block))
    }

    pub fn get_quant_table(&self, component: u8) -> &[i16; 64] {
        self.parser.get_quant_table(component)
    }

    pub fn get_component_table(&self, component: u8) -> ComponentTable {
        self.parser.get_component_table(component)
    }

    pub fn get_prime_component(&self) -> u8 {
        self.parser.prime_component
    }

    fn update_state<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
    ) -> Result<(usize, u8, u8, u8), JpegDecoderError> {
        if self.state.strm_end {
            return Err(JpegDecoderError::StateError);
        }

        let (mcu, comp, v, h) = (self.state.mcu, self.state.comp, self.state.v, self.state.h);
        // capture the current state

        let ComponentTable {
            horizontal_sampling_factor,
            vertical_sampling_factor,
            ..
        } = self.get_component_table(comp);

        // try to update
        self.state.h += 1;
        if self.state.h >= horizontal_sampling_factor {
            self.state.h = 0;

            self.state.v += 1;
            if self.state.v >= vertical_sampling_factor {
                self.state.v = 0;

                self.state.comp += 1;
                if self.state.comp >= self.state.max_comp {
                    self.state.comp = self.state.min_comp;

                    self.state.mcu += 1;
                    if self.state.mcu >= self.parser.mcu.total_mcu {
                        // parse will also reset our state
                        match self.parse(reader, &mut None) {
                            Ok(Some(())) => {}
                            Ok(None) => self.state.strm_end = true, // no more update available so we set strm_end here
                            Err(e) => return Err(e),
                        }
                    }
                }
            }
        }

        Ok((mcu, comp, v, h))
    }

    fn parse<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        image: &mut Option<&mut Image>,
    ) -> Result<Option<()>, JpegDecoderError> {
        const LIMIT: usize = 128;
        let mut i = 0;
        while i < LIMIT {
            let marker = self.parser.get_marker(reader)?;

            if marker.marker_type == EOI {
                return Ok(None);
            }

            self.parser
                .parse_marker(reader, marker, image, &mut self.state)?;

            if marker.marker_type == SOS {
                self.huffman = HuffmanDecoder::new(reader)?;
                return Ok(Some(()));
            }
            i += 1;
        }

        return Err(JpegDecoderError::NoMarker);
    }
}

impl JpegDecoder {
    // write an 8x8 block
    // this will clamp(0, 255) Block's data
    pub fn write_block<T>(&self, image: &mut Image, block: &Block<T>)
    where
        T: Clone + Copy + From<u8> + cmp::Ord + TryInto<u8>,
    {
        let component = block.component as usize;
        let p = block.mcu as usize;
        let bpp: usize = image.channels as usize;
        let bpr: usize = image.width as usize * bpp;

        // i_factor and j_factor tell how much 1 component should scale up
        // For example: 4Y 1Cb 1Cr,
        // (i_factor, j_factor) | component
        // (1, 1) | Y
        // (2, 2) | Cb
        // (2, 2) | Cr
        let i_factor = self.get_component_table(component as u8).i_factor;
        let j_factor = self.get_component_table(component as u8).j_factor;

        let row = (p / self.parser.mcu.mcu_per_row) * self.parser.mcu.mcu_height
            + (block.v as usize) * 8 * i_factor;
        let col = (p % self.parser.mcu.mcu_per_row) * self.parser.mcu.mcu_width
            + (block.h as usize) * 8 * j_factor;

        let mut i: usize = 0;
        while i < 8 * i_factor && row + i < image.height as usize {
            let mut j: usize = 0;
            while j < 8 * j_factor && col + j < image.width as usize {
                let scaled_i = i / i_factor;
                let scaled_j = j / j_factor;

                let block_index = scaled_i * 8 + scaled_j;
                let pixel_index = (row + i) * bpr + (col + j) * bpp + component;

                image.pixels[pixel_index] = match block.data[block_index]
                    .clamp(0.into(), 0xFF.into())
                    .try_into()
                {
                    Ok(x) => x,
                    Err(_) => panic!(), // this will never happen because the value is clamped
                };

                j += 1;
            }
            i += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JpegDecoderError {
    ReadFail(io::ErrorKind),
    NoMarker,
    MarkerInvalid,
    EarlyEOI,
    NotAJPEG,

    UnknownSymbol,
    UnknownMarker(u8),

    StateError,
}

impl From<ParserError> for JpegDecoderError {
    fn from(value: ParserError) -> Self {
        match value {
            ParserError::EarlyEOI => JpegDecoderError::EarlyEOI,
            ParserError::MarkerInvalid => JpegDecoderError::MarkerInvalid,
            ParserError::NoMarker => JpegDecoderError::NoMarker,
            ParserError::NotAJPEG => JpegDecoderError::NotAJPEG,
            ParserError::ReadFail(v) => JpegDecoderError::ReadFail(v),
        }
    }
}

impl From<HuffmanDecoderError> for JpegDecoderError {
    fn from(value: HuffmanDecoderError) -> Self {
        match value {
            HuffmanDecoderError::ReadFail(v) => JpegDecoderError::ReadFail(v),
            HuffmanDecoderError::UnknownSymbol => JpegDecoderError::UnknownSymbol,
            HuffmanDecoderError::Marker(m) => JpegDecoderError::UnknownMarker(m),
            HuffmanDecoderError::StreamError => JpegDecoderError::StateError,
        }
    }
}

impl From<io::Error> for JpegDecoderError {
    fn from(value: io::Error) -> Self {
        Self::ReadFail(value.kind())
    }
}

impl<T> Default for Block<T>
where
    T: Clone + Copy + From<u8>,
{
    fn default() -> Self {
        Self {
            data: [0.into(); 64],
            mcu: 0,
            component: 0,
            v: 0,
            h: 0,
        }
    }
}
