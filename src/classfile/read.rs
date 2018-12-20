use super::constant;
use std::fs::File;
use std::io::{BufReader, Read};

#[derive(Debug)]
pub struct ClassFileReader {
    reader: BufReader<File>,
}

macro_rules! try_eq {
    ($expr:expr) => {{
        if !$expr {
            return None;
        }
    }};
}

impl ClassFileReader {
    pub fn new(filename: &str) -> Option<Self> {
        let file = match File::open(filename) {
            Ok(file) => file,
            Err(_) => return None,
        };

        Some(ClassFileReader {
            reader: BufReader::new(file),
        })
    }

    pub fn read(&mut self) -> Option<()> {
        let magic = self.read_u32()?;
        try_eq!(magic == 0xCAFEBABE);
        println!("cafebabe!");

        let minor_version = self.read_u16()?;
        let major_version = self.read_u16()?;
        println!(
            "version: minor: {}, major: {}",
            minor_version, major_version
        );

        let constant_pool_count = self.read_u16()?;
        println!("constant_pool_count: {}", constant_pool_count);

        // loop {
        let tag = self.read_u8()?;
        println!("tag: {:?}", constant::u8_to_constant_type(tag)?);
        // }

        Some(())
    }

    fn read_u32(&mut self) -> Option<u32> {
        let mut buf = [0u8; 4];
        match self.reader.read(&mut buf) {
            Ok(sz) => {
                assert_eq!(sz, 4);
                Some(
                    ((buf[0] as u32) << 24)
                        + ((buf[1] as u32) << 16)
                        + ((buf[2] as u32) << 8)
                        + buf[3] as u32,
                )
            }
            Err(_) => None,
        }
    }

    fn read_u16(&mut self) -> Option<u16> {
        let mut buf = [0u8; 2];
        match self.reader.read(&mut buf) {
            Ok(sz) => {
                assert_eq!(sz, 2);
                Some(((buf[0] as u16) << 8) + buf[1] as u16)
            }
            Err(_) => None,
        }
    }

    fn read_u8(&mut self) -> Option<u8> {
        let mut buf = [0u8; 1];
        match self.reader.read(&mut buf) {
            Ok(sz) => {
                assert_eq!(sz, 1);
                Some(buf[0])
            }
            Err(_) => None,
        }
    }
}
