use std::io;

use crate::EOI;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BitReader {
    pub buf: u16,
    head: u8,
    head_len: u8,

    pub buf_len: u8,
    err_head: BitReaderError,
}

impl BitReader {
    pub fn new<R: io::Read + io::Seek>(reader: &mut R) -> Result<Self, BitReaderError> {
        let mut v = Self {
            buf: 0,
            head: 0,
            head_len: 0,
            buf_len: 16,
            err_head: BitReaderError::default(),
        };
        v.read_bits(reader, 16)?;
        Ok(v)
    }

    // consume bits from buf and return the value
    pub fn read_bits<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        bits: u8,
    ) -> Result<u16, BitReaderError> {
        let ret: u16 = self.buf.unbounded_shr(16 - bits as u32);

        if self.head_len >= bits {
            self.pull_head(bits)?;
            return Ok(ret);
        }

        let head_len = self.head_len;
        self.pull_head(head_len)?;
        let remaining_bits = bits - head_len;
        self.fetch_head(reader)?;
        self.read_bits(reader, remaining_bits)?;

        Ok(ret)
    }

    // consume bits from head and push to buf
    fn pull_head(&mut self, bits: u8) -> Result<(), BitReaderError> {
        self.buf = self.buf.unbounded_shl(bits as u32);

        self.buf_len = match self.err_head {
            BitReaderError::Marker(_) => self.buf_len.checked_sub(bits).ok_or(self.err_head),
            _ => Ok(self.buf_len),
        }?;

        self.buf |= (self.head.unbounded_shr(8 - bits as u32)) as u16;
        self.head = self.head.unbounded_shl(bits as u32);
        self.head_len -= bits;

        Ok(())
    }

    // overwrite head with the next byte from file
    fn fetch_head<R: io::Read + io::Seek>(&mut self, reader: &mut R) -> Result<(), BitReaderError> {
        let mut buf: [u8; 1] = [0];
        self.read(reader, &mut buf)?;
        self.head = buf[0];
        self.head_len = 8;

        if self.head == 0xFF {
            self.read(reader, &mut buf)?;

            if buf[0] != 0x00 {
                // This is a marker, we seek back to preserve the state
                self.seek(reader, -2)?;
                self.err_head = BitReaderError::Marker(buf[0]);
            }
        }

        Ok(())
    }

    // we probably don't wanna call the reader directly
    fn read<R: io::Read>(&mut self, reader: &mut R, buf: &mut [u8]) -> Result<(), BitReaderError> {
        match reader
            .read_exact(buf)
            .map_err(|e| BitReaderError::ReadFail(e.kind()))
        {
            Ok(_) => Ok(()),
            Err(BitReaderError::ReadFail(io::ErrorKind::UnexpectedEof)) => {
                if buf.len() > 0 {
                    buf[0] = EOI;
                    Ok(())
                } else {
                    Err(BitReaderError::ReadFail(io::ErrorKind::UnexpectedEof))
                }
            }
            el => el,
        }
    }

    fn seek<R: io::Read + io::Seek>(
        &mut self,
        reader: &mut R,
        offset: i64,
    ) -> Result<(), BitReaderError> {
        reader
            .seek_relative(offset)
            .map_err(|e| BitReaderError::ReadFail(e.kind()))
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BitReaderError {
    ReadFail(io::ErrorKind),
    Marker(u8),
    #[default]
    InputError, // read_bits can only read 16 bit
}

impl From<io::Error> for BitReaderError {
    fn from(value: io::Error) -> Self {
        Self::ReadFail(value.kind())
    }
}

impl From<BitReaderError> for () {
    fn from(_: BitReaderError) -> Self {
        ()
    }
}
