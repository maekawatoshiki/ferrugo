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
                        + (buf[3] as u32),
                )
            }
            Err(_) => None,
        }
    }
}
