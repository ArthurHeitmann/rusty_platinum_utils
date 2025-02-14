use std::io::{Read, Seek};
use byteorder::{LittleEndian, ReadBytesExt};
use half::f16;

pub struct ByteReader<R: Read + Seek> {
    reader: R,
}

impl<R: Read + Seek> ByteReader<R> {
    pub fn new(reader: R) -> Self {
        ByteReader { reader }
    }

    pub fn seek(&mut self, pos: u64) -> Result<u64, String> {
        self.reader.seek(std::io::SeekFrom::Start(pos)).map_err(|e| e.to_string())
    }

    pub fn size(&mut self) -> Result<u64, String> {
        let pos = self.reader.seek(std::io::SeekFrom::Current(0)).map_err(|e| e.to_string())?;
        let size = self.reader.seek(std::io::SeekFrom::End(0)).map_err(|e| e.to_string())?;
        self.reader.seek(std::io::SeekFrom::Start(pos)).map_err(|e| e.to_string())?;
        Ok(size)
    }

    pub fn read(&mut self, size: usize) -> Result<Vec<u8>, String> {
        let mut buffer = vec![0; size];
        self.reader.read_exact(&mut buffer).map_err(|e| e.to_string())?;
        Ok(buffer)
    }

    pub fn position(&mut self) -> Result<u64, String> {
        self.reader.seek(std::io::SeekFrom::Current(0)).map_err(|e| e.to_string())
    }

    pub fn read_u8(&mut self) -> Result<u8, String> {
        self.reader.read_u8().map_err(|e| e.to_string())
    }

    pub fn read_i8(&mut self) -> Result<i8, String> {
        self.reader.read_i8().map_err(|e| e.to_string())
    }

    pub fn read_u16(&mut self) -> Result<u16, String> {
        self.reader.read_u16::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_i16(&mut self) -> Result<i16, String> {
        self.reader.read_i16::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_u32(&mut self) -> Result<u32, String> {
        self.reader.read_u32::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_i32(&mut self) -> Result<i32, String> {
        self.reader.read_i32::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_u64(&mut self) -> Result<u64, String> {
        self.reader.read_u64::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_i64(&mut self) -> Result<i64, String> {
        self.reader.read_i64::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_f32(&mut self) -> Result<f32, String> {
        self.reader.read_f32::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_f16(&mut self) -> Result<f16, String> {
        let mut buf: [u8; 2] = [0; 2];
        self.reader.read_exact(&mut buf).map_err(|e| e.to_string())?;
        Ok(f16::from_le_bytes(buf))
    }

    pub fn read_f64(&mut self) -> Result<f64, String> {
        self.reader.read_f64::<LittleEndian>().map_err(|e| e.to_string())
    }

    pub fn read_string(&mut self, count: usize) -> Result<String, String> {
        let mut buffer = vec![0; count];
        self.reader.read_exact(&mut buffer).map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }

    pub fn read_string_zero_term(&mut self) -> Result<String, String> {
        let mut buffer = Vec::new();
        loop {
            let byte = self.read_u8()?;
            if byte == 0 {
                break;
            }
            buffer.push(byte);
        }
        Ok(String::from_utf8_lossy(&buffer).to_string())
    }
}
