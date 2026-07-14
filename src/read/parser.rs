use std::io;

use crate::{DHT, DQT, EOI, Image, SOF0, SOI, SOS, read::JpegDecoderState};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct JpegParser {
    check_list: CheckList,

    quant_table: [Vec<i16>; 4],
    component_table: [ComponentTable; 3],
    huffman_table: [[Vec<HuffmanSymbol>; 2]; 2],

    pub mcu: Mcu,
    pub prime_component: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentTable {
    pub component_id: u8,
    pub horizontal_sampling_factor: u8,
    pub vertical_sampling_factor: u8,
    pub i_factor: usize,
    pub j_factor: usize,
    pub quant_id: u8,
    pub huffman_id: [u8; 2],
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HuffmanSymbol {
    pub symbol: u8,
    pub code_len: u8, // we're relying on code_len of 0 to be non-exist code
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Mcu {
    pub mcu_width: usize,
    pub mcu_height: usize,

    pub mcu_per_row: usize,
    pub mcu_per_col: usize,

    pub total_mcu: usize,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)] // Default of bool is false
pub struct CheckList {
    pub soi: bool,
    pub dqt: bool,
    pub sof0: bool,
    pub dht: bool,
    pub sos: bool,
}

impl JpegParser {
    pub fn get_quant_table(&self, component: u8) -> &[i16; 64] {
        self.quant_table[self.component_table[component as usize].quant_id as usize][0..64]
            .try_into()
            .unwrap()
    }

    pub fn get_huffman_table(&self, component: u8) -> [&[HuffmanSymbol; 65536]; 2] {
        let huffman_id = self.component_table[component as usize].huffman_id;

        #[rustfmt::skip]
        let (huffman_table_dc, huffman_table_ac): (&[HuffmanSymbol; 65536], &[HuffmanSymbol; 65536]) = (
            self.huffman_table[0][huffman_id[0] as usize][0..65536].try_into().unwrap(),
            self.huffman_table[1][huffman_id[1] as usize][0..65536].try_into().unwrap()
        );

        [huffman_table_dc, huffman_table_ac]
    }

    pub fn get_component_table(&self, component: u8) -> ComponentTable {
        self.component_table[component as usize]
    }

    pub fn get_marker<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
    ) -> Result<Marker, ParserError> {
        let mut buf: [u8; 4] = [0; 4];

        const LIMIT: usize = 128;
        let mut i = 0;
        while buf[0] != 0xFF && i < LIMIT {
            self.read(reader, &mut buf[0..1])?;
            i += 1;
        }

        self.read(reader, &mut buf[1..2])?;

        match buf[1] {
            EOI => {
                return Ok(Marker {
                    marker_type: EOI,
                    marker_len: 0,
                });
            }

            SOI => {
                return Ok(Marker {
                    marker_type: SOI,
                    marker_len: 0,
                });
            }

            _ => {}
        }

        self.read(reader, &mut buf[2..4])?;

        Ok(Marker {
            marker_type: buf[1],
            marker_len: u16::from_be_bytes(buf[2..4].try_into().unwrap()),
        })
    }

    pub fn parse_marker<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker: Marker,
        image: &mut Option<&mut Image>,
        state: &mut JpegDecoderState,
    ) -> Result<(), ParserError> {
        let Marker {
            marker_type,
            marker_len,
        } = marker;

        if marker_type == SOI {
            self.check_list.soi = true;
            return Ok(());
        }

        if !self.check_list.soi {
            return Err(ParserError::NotAJPEG);
        }

        match marker_type {
            DQT => self.dqt_parse(reader, marker_len)?,
            DHT => self.dht_parse(reader, marker_len)?,
            SOS => self.sos_parse(reader, marker_len, state)?,
            SOF0 if let Some(img) = image => self.sof0_parse(reader, marker_len, *img)?,
            EOI => return Err(ParserError::EarlyEOI),
            _ => self.skip_marker(reader, marker)?,
        }

        Ok(())
    }

    pub fn skip_marker<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker: Marker,
    ) -> Result<(), ParserError> {
        if marker.marker_len == 0 {
            return Err(ParserError::MarkerInvalid);
        }
        self.seek(reader, marker.marker_len as i64 - 2)
    }

    fn dqt_parse<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker_len: u16,
    ) -> Result<(), ParserError> {
        let mut marker_len = marker_len - 2;

        let bit_depth: u8;
        let destination: u8;
        {
            let mut buf: [u8; 1] = [0];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            bit_depth = match (buf[0] >> 4) * 0x0F {
                0 => 8,
                1 => 16,
                _ => return Err(ParserError::MarkerInvalid),
            };

            destination = (buf[0] >> 0) & 0x0F;
            match destination {
                0..4 => {}
                _ => return Err(ParserError::MarkerInvalid),
            };
        }

        let quant_table_len: usize = match bit_depth {
            8 => 64,
            16 => 128,
            _ => return Err(ParserError::MarkerInvalid),
        };

        // baseline JPEG
        if bit_depth != 8 {
            return Err(ParserError::MarkerInvalid);
        }

        let mut buf: Vec<u8> = vec![0; quant_table_len];
        self.read(reader, &mut buf)?;

        marker_len = marker_len
            .checked_sub(buf.len() as u16)
            .ok_or(ParserError::MarkerInvalid)?;

        self.quant_table[destination as usize] = vec![0; buf.len()];
        for i in 0..buf.len() {
            self.quant_table[destination as usize][i] = buf[i] as i16;
        }

        if marker_len != 0 {
            return Err(ParserError::MarkerInvalid);
        }
        self.check_list.dqt = true;

        Ok(())
    }

    fn dht_parse<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker_len: u16,
    ) -> Result<(), ParserError> {
        let mut marker_len = marker_len - 2;

        while marker_len > 0 {
            let destination: u8;
            let class: u8;
            {
                let mut buf: [u8; 1] = [0];
                self.read(reader, &mut buf)?;

                marker_len = marker_len
                    .checked_sub(buf.len() as u16)
                    .ok_or(ParserError::MarkerInvalid)?;

                destination = (buf[0] >> 0) & 0x0F;
                class = (buf[0] >> 4) & 0x0F;

                match destination {
                    0 | 1 => {}
                    _ => return Err(ParserError::MarkerInvalid),
                }
                match class {
                    0 | 1 => {} // DC | AC
                    _ => return Err(ParserError::MarkerInvalid),
                }
            }

            let mut code_counts: [u8; 16] = [0; 16];
            self.read(reader, &mut code_counts)?;

            marker_len = marker_len
                .checked_sub(code_counts.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            let total_symbol: usize;
            {
                let mut sum: usize = 0;
                for i in 0..code_counts.len() {
                    sum += code_counts[i] as usize;
                }
                if sum > 256 {
                    return Err(ParserError::MarkerInvalid);
                }
                total_symbol = sum;
            }

            let mut symbols: Vec<u8> = vec![0; total_symbol]; // decoded symbols
            self.read(reader, &mut symbols)?;

            marker_len = marker_len
                .checked_sub(symbols.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            let class: usize = class as usize;
            let destination: usize = destination as usize;

            self.huffman_table[class][destination] = vec![
                HuffmanSymbol {
                    symbol: 0,
                    code_len: 0
                };
                65536
            ];
            let table = &mut self.huffman_table[class][destination];

            let mut code: u16 = 0;
            let mut sym_count: usize = 0;

            for len in 1..=16 {
                for _ in 0..code_counts[len - 1] {
                    table[code as usize] = HuffmanSymbol {
                        symbol: symbols[sym_count],
                        code_len: len as u8,
                    };
                    sym_count += 1;
                    code = code.wrapping_add(1);
                }
                code <<= 1;
            }
        }

        self.check_list.dht = true;
        Ok(())
    }

    fn sof0_parse<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker_len: u16,
        image: &mut Image,
    ) -> Result<(), ParserError> {
        let mut marker_len = marker_len - 2;

        if self.check_list.sof0 {
            return Err(ParserError::MarkerInvalid);
        }

        {
            let mut buf: [u8; 6] = [0; 6];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            image.bit_depth = buf[0];
            image.height = u16::from_be_bytes(buf[1..=2].try_into().unwrap());
            image.width = u16::from_be_bytes(buf[3..=4].try_into().unwrap());
            image.channels = buf[5];

            match image.bit_depth {
                8 | 16 => {}
                _ => return Err(ParserError::MarkerInvalid),
            }

            match image.channels {
                1 | 3 => {}
                _ => return Err(ParserError::MarkerInvalid),
            }
        }

        // baseline JPEG
        if image.bit_depth != 8 {
            return Err(ParserError::MarkerInvalid);
        }

        if marker_len != 3 * image.channels as u16 {
            return Err(ParserError::MarkerInvalid);
        }

        let mut v: Vec<usize> = vec![0; image.channels as usize];
        let mut h: Vec<usize> = vec![0; image.channels as usize];

        self.prime_component = 0;
        for i in 0..image.channels as usize {
            let mut buf: [u8; 3] = [0; 3];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            self.component_table[i] = ComponentTable {
                component_id: match buf[0] {
                    1..=3 => buf[0],
                    _ => return Err(ParserError::MarkerInvalid),
                },
                horizontal_sampling_factor: (buf[1] >> 4) & 0x0F,
                vertical_sampling_factor: (buf[1] >> 0) & 0x0F,
                quant_id: match buf[2] {
                    0..4 => buf[2],
                    _ => return Err(ParserError::MarkerInvalid),
                },
                ..self.component_table[i]
            };

            h[i] = self.component_table[i].horizontal_sampling_factor as usize;
            v[i] = self.component_table[i].vertical_sampling_factor as usize;

            if h[i] > h[self.prime_component as usize] {
                self.prime_component = i as u8;
            }
        }

        let vmax = v[self.prime_component as usize];
        let hmax = h[self.prime_component as usize];

        if vmax == 0 || hmax == 0 {
            return Err(ParserError::MarkerInvalid);
        }

        for i in 0..image.channels as usize {
            self.component_table[i] = ComponentTable {
                i_factor: vmax / v[i],
                j_factor: hmax / h[i],
                ..self.component_table[i]
            };
        }

        self.mcu.mcu_width = hmax * 8;
        self.mcu.mcu_height = vmax * 8;

        self.mcu.mcu_per_row = (image.width as usize).div_ceil(self.mcu.mcu_width);
        self.mcu.mcu_per_col = (image.height as usize).div_ceil(self.mcu.mcu_height);

        self.mcu.total_mcu = self.mcu.mcu_per_row * self.mcu.mcu_per_col;

        if marker_len != 0 {
            return Err(ParserError::MarkerInvalid);
        }

        self.check_list.sof0 = true;
        Ok(())
    }

    fn sos_parse<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        marker_len: u16,
        state: &mut JpegDecoderState,
    ) -> Result<(), ParserError> {
        let mut marker_len = marker_len - 2;

        if self.check_list.dqt == false
            || self.check_list.sof0 == false
            || self.check_list.dht == false
        {
            return Err(ParserError::MarkerInvalid);
        }

        let component_count: usize = {
            let mut buf: [u8; 1] = [0];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            Ok::<usize, ParserError>(match buf[0] {
                1 | 3 => buf[0] as usize,
                _ => return Err(ParserError::MarkerInvalid),
            })
        }?;

        if component_count == 1 {
            let mut buf: [u8; 2] = [0; 2];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            let mut comp: Option<u8> = None;
            for j in 0..self.component_table.len() {
                if buf[0] == self.component_table[j].component_id {
                    comp = Some(j as u8);
                    break;
                }
            }
            if comp == None {
                return Err(ParserError::MarkerInvalid);
            }
            let comp = comp.unwrap();

            // [DC, AC]
            self.component_table[comp as usize].huffman_id =
                [(buf[1] >> 4) & 0x0F, (buf[1] >> 0) & 0x0F];

            *state = JpegDecoderState {
                mcu: 0,
                max_comp: comp + 1,
                comp: comp,
                min_comp: comp,
                h: 0,
                v: 0,
                ..*state
            };
        } else {
            if *state != JpegDecoderState::default() {
                return Err(ParserError::MarkerInvalid);
            }

            for i in 0..component_count {
                let mut buf: [u8; 2] = [0; 2];
                self.read(reader, &mut buf)?;

                marker_len = marker_len
                    .checked_sub(buf.len() as u16)
                    .ok_or(ParserError::MarkerInvalid)?;

                if buf[0] as usize != i + 1 {
                    return Err(ParserError::MarkerInvalid);
                }

                // [DC, AC]
                self.component_table[i].huffman_id = [(buf[1] >> 4) & 0x0F, (buf[1] >> 0) & 0x0F];
            }

            *state = JpegDecoderState {
                mcu: 0,
                max_comp: component_count as u8,
                comp: 0,
                min_comp: 0,
                h: 0,
                v: 0,
                ..*state
            };
        }

        {
            let mut buf: [u8; 3] = [0; 3];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(ParserError::MarkerInvalid)?;

            if buf != [0x00, 0x3F, 0x00] {
                return Err(ParserError::MarkerInvalid);
            }
        }

        if marker_len != 0 {
            return Err(ParserError::MarkerInvalid);
        }

        Ok(())
    }

    fn read<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        buf: &mut [u8],
    ) -> Result<(), ParserError> {
        match reader
            .read_exact(buf)
            .map_err(|e| ParserError::ReadFail(e.kind()))
        {
            Ok(_) => Ok(()),
            Err(ParserError::ReadFail(io::ErrorKind::UnexpectedEof)) => {
                if buf.len() == 1 {
                    buf[0] = EOI;
                    Ok(())
                } else {
                    Err(ParserError::ReadFail(io::ErrorKind::UnexpectedEof))
                }
            }
            el => el,
        }
    }

    fn seek<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        offset: i64,
    ) -> Result<(), ParserError> {
        reader
            .seek_relative(offset)
            .map_err(|e| ParserError::ReadFail(e.kind()))
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Marker {
    pub marker_type: u8,
    pub marker_len: u16,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ParserError {
    ReadFail(io::ErrorKind),
    NoMarker, // get_marker is called, but no marker found
    //UnknownMarker,
    #[default]
    MarkerInvalid,
    EarlyEOI,
    NotAJPEG,
}

impl From<ParserError> for () {
    fn from(_: ParserError) -> Self {
        ()
    }
}

impl Default for JpegParser {
    fn default() -> Self {
        Self {
            check_list: CheckList::default(),
            quant_table: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            component_table: [ComponentTable::default(); 3],
            huffman_table: [[Vec::new(), Vec::new()], [Vec::new(), Vec::new()]],
            mcu: Mcu::default(),
            prime_component: 0,
        }
    }
}

/*
    fn sos_parse<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        header: &mut JpegParser,
    ) -> Result<Option<()>, JpegDecoderError> {
        let (mut marker_type, mut marker_len): (u8, u16) = self.get_marker(reader)?;

        while marker_type != SOS {
            match marker_type {
                DHT => header.dht_parse(reader, marker_len)?,
                DQT => header.dqt_parse(reader, marker_len)?,
                EOI => return Ok(None),
                _ => return Err(JpegDecoderError::MarkerInvalid),
            }

            (marker_type, marker_len) = self.get_marker(reader)?;
        }

        marker_len -= 2;

        let component_count: usize = {
            let mut buf: [u8; 1] = [0];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(JpegDecoderError::MarkerInvalid)?;

            Ok::<usize, JpegDecoderError>(match buf[0] {
                1 | 3 => buf[0] as usize,
                _ => return Err(JpegDecoderError::MarkerInvalid),
            })
        }?;

        if component_count == 1 {
            let mut buf: [u8; 2] = [0; 2];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(JpegDecoderError::MarkerInvalid)?;

            let mut comp: Option<u8> = None;
            for j in 0..header.component_table.len() {
                if buf[0] == header.component_table[j].component_id {
                    comp = Some(j as u8);
                    break;
                }
            }
            if comp == None {
                return Err(JpegDecoderError::MarkerInvalid);
            }
            let comp = comp.unwrap();

            // [DC, AC]
            header.component_table[comp as usize].huffman_id =
                [(buf[1] >> 4) & 0x0F, (buf[1] >> 0) & 0x0F];

            *self = JpegDecoderState {
                mcu: 0,
                max_comp: comp + 1,
                comp: comp,
                min_comp: comp,
                h: 0,
                v: 0,
            };
        } else {
            if *self != JpegDecoderState::default() {
                return Err(JpegDecoderError::MarkerInvalid);
            }

            for i in 0..component_count {
                let mut buf: [u8; 2] = [0; 2];
                self.read(reader, &mut buf)?;

                marker_len = marker_len
                    .checked_sub(buf.len() as u16)
                    .ok_or(JpegDecoderError::MarkerInvalid)?;

                if buf[0] as usize != i + 1 {
                    return Err(JpegDecoderError::MarkerInvalid);
                }

                // [DC, AC]
                header.component_table[i].huffman_id = [(buf[1] >> 4) & 0x0F, (buf[1] >> 0) & 0x0F];
            }

            *self = JpegDecoderState {
                mcu: 0,
                max_comp: component_count as u8,
                comp: 0,
                min_comp: 0,
                h: 0,
                v: 0,
            };
        }

        {
            let mut buf: [u8; 3] = [0; 3];
            self.read(reader, &mut buf)?;

            marker_len = marker_len
                .checked_sub(buf.len() as u16)
                .ok_or(JpegDecoderError::MarkerInvalid)?;

            if buf != [0x00, 0x3F, 0x00] {
                return Err(JpegDecoderError::MarkerInvalid);
            }
        }

        if marker_len != 0 {
            return Err(JpegDecoderError::MarkerInvalid);
        }

        Ok(Some(()))
    }

    fn read<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
        buf: &mut [u8],
    ) -> Result<(), JpegDecoderError> {
        reader
            .read_exact(buf)
            .map_err(|e| JpegDecoderError::ReadFail(e.kind()))
    }

    fn get_marker<T: io::Read + io::Seek>(
        &mut self,
        reader: &mut T,
    ) -> Result<(u8, u16), JpegDecoderError> {
        let mut buf: [u8; 4] = [0; 4];

        self.read(reader, &mut buf[0..1])?;
        if buf[0] != 0xFF {
            while buf[0] != 0xFF {
                self.read(reader, &mut buf[0..1])?;
            }
            //return Err(JpegDecoderError::MarkerInvalid);
        }

        self.read(reader, &mut buf[1..2])?;
        if buf[1] != EOI {
            self.read(reader, &mut buf[2..4])?;
        }

        Ok((buf[1], u16::from_be_bytes(buf[2..4].try_into().unwrap())))
    }
*/
