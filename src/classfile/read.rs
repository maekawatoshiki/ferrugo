use super::constant;
use super::constant::{Constant, ConstantType};
use std::fs::File;
use std::io::{BufReader, Read};
use std::mem::transmute;

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

        let mut idx = 0;
        while idx < constant_pool_count - 1 {
            let tag = self.read_u8()?;
            // println!("tag: {:?}", tag);
            let const_ty = constant::u8_to_constant_type(tag)?;
            println!("tag: {:?}", const_ty);
            println!("-> {:?}", self.read_constant(&const_ty)?);

            // https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.4.5
            // > If a CONSTANT_Long_info or CONSTANT_Double_info structure is the item in the
            // > constant_pool table at index n, then the next usable item in the pool is located at
            // > index n+2. The constant_pool index n+1 must be valid but is considered unusable.
            match const_ty {
                ConstantType::Double | ConstantType::Long => idx += 2,
                _ => idx += 1,
            }
        }

        Some(())
    }
}

// Constants

impl ClassFileReader {
    fn read_constant(&mut self, ty: &ConstantType) -> Option<Constant> {
        match ty {
            ConstantType::Methodref => self.read_constant_methodref_info(),
            ConstantType::Fieldref => self.read_constant_fieldref_info(),
            ConstantType::InterfaceMethodref => self.read_constant_interface_methodref_info(),
            ConstantType::String => self.read_constant_string(),
            ConstantType::Class => self.read_constant_class_info(),
            ConstantType::Utf8 => self.read_constant_utf8(),
            ConstantType::NameAndType => self.read_constant_name_and_type_info(),
            ConstantType::Integer => self.read_constant_integer_info(),
            ConstantType::Float => self.read_constant_float_info(),
            ConstantType::Long => self.read_constant_long_info(),
            ConstantType::Double => self.read_constant_double_info(),
            ConstantType::MethodHandle => self.read_constant_method_handle_info(),
            ConstantType::MethodType => self.read_constant_method_type_info(),
            ConstantType::InvokeDynamic => self.read_constant_invoke_dynamic_info(),
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

    fn read_constant_name_and_type_info(&mut self) -> Option<Constant> {
        let name_index = self.read_u16()?;
        let descriptor_index = self.read_u16()?;
        Some(Constant::NameAndTypeInfo {
            name_index,
            descriptor_index,
        })
    }

    fn read_constant_string(&mut self) -> Option<Constant> {
        let string_index = self.read_u16()?;
        Some(Constant::String { string_index })
    }

    fn read_constant_class_info(&mut self) -> Option<Constant> {
        let name_index = self.read_u16()?;
        Some(Constant::ClassInfo { name_index })
    }

    fn read_constant_utf8(&mut self) -> Option<Constant> {
        let length = self.read_u16()?;
        let mut bytes = vec![];
        for _ in 0..length {
            bytes.push(self.read_u8()?);
        }
        Some(Constant::Utf8 {
            s: String::from_utf8(bytes).ok()?,
        })
    }

    fn read_constant_integer_info(&mut self) -> Option<Constant> {
        let bytes = self.read_u32()?;
        Some(Constant::IntegerInfo { i: bytes as i32 })
    }

    fn read_constant_float_info(&mut self) -> Option<Constant> {
        let bytes = self.read_u32()?;
        Some(Constant::FloatInfo {
            f: unsafe { transmute::<u32, f32>(bytes) },
        })
    }

    fn read_constant_long_info(&mut self) -> Option<Constant> {
        let high_bytes = self.read_u32()?;
        let low_bytes = self.read_u32()?;
        Some(Constant::LongInfo {
            i: ((high_bytes as i64) << 32) + low_bytes as i64,
        })
    }

    fn read_constant_double_info(&mut self) -> Option<Constant> {
        let high_bytes = self.read_u32()?;
        let low_bytes = self.read_u32()?;
        Some(Constant::DoubleInfo {
            f: unsafe { transmute::<u64, f64>(((high_bytes as u64) << 32) + low_bytes as u64) },
        })
    }

    fn read_constant_method_handle_info(&mut self) -> Option<Constant> {
        let reference_kind = self.read_u8()?;
        let reference_index = self.read_u16()?;
        Some(Constant::MethodHandleInfo {
            reference_kind,
            reference_index,
        })
    }

    fn read_constant_method_type_info(&mut self) -> Option<Constant> {
        let descriptor_index = self.read_u16()?;
        Some(Constant::MethodTypeInfo { descriptor_index })
    }
    fn read_constant_invoke_dynamic_info(&mut self) -> Option<Constant> {
        let bootstrap_method_attr_index = self.read_u16()?;
        let name_and_type_index = self.read_u16()?;
        Some(Constant::InvokeDynamicInfo {
            bootstrap_method_attr_index,
            name_and_type_index,
        })
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
