use super::constant;
use super::constant::{Constant, ConstantType};
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

        for i in 0..5 {
            let tag = self.read_u8()?;
            let const_ty = constant::u8_to_constant_type(tag)?;
            println!("tag: {:?}", const_ty);
            println!("data: {:?}", self.read_constant(const_ty)?);
        }

        Some(())
    }
}

// Constants

impl ClassFileReader {
    fn read_constant(&mut self, ty: ConstantType) -> Option<Constant> {
        match ty {
            ConstantType::Methodref => self.read_constant_methodref_info(),
            ConstantType::Fieldref => self.read_constant_fieldref_info(),
            ConstantType::InterfaceMethodref => self.read_constant_interface_methodref_info(),
            ConstantType::String => self.read_constant_string(),
            _ => None,
        }
    }

    fn read_constant_methodref_info(&mut self) -> Option<Constant> {
        let class_index = self.read_u16()?;
        let name_and_type_index = self.read_u16()?;
        Some(Constant::MethodrefInfo {
            class_index,
            name_and_type_index,
        })
    }

    fn read_constant_fieldref_info(&mut self) -> Option<Constant> {
        let class_index = self.read_u16()?;
        let name_and_type_index = self.read_u16()?;
        Some(Constant::FieldrefInfo {
            class_index,
            name_and_type_index,
        })
    }

    fn read_constant_interface_methodref_info(&mut self) -> Option<Constant> {
        let class_index = self.read_u16()?;
        let name_and_type_index = self.read_u16()?;
        Some(Constant::InterfaceMethodrefInfo {
            class_index,
            name_and_type_index,
        })
    }

    fn read_constant_string(&mut self) -> Option<Constant> {
        let string_index = self.read_u16()?;
        Some(Constant::String { string_index })
    }
}

// Utils

impl ClassFileReader {
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
