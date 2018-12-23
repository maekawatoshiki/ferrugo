use super::attribute::{
    Annotation, Attribute, AttributeInfo, ElementValue, ElementValuePair, Exception,
    InnerClassesBody, LineNumber, StackMapFrame, StackMapFrameBody, VerificationTypeInfo,
};
use super::classfile::ClassFile;
use super::constant;
use super::constant::{Constant, ConstantType};
use super::field::FieldInfo;
use super::method::MethodInfo;
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

    pub fn read(&mut self) -> Option<ClassFile> {
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

        let mut constant_pool = vec![Constant::None];
        let mut idx = 0;
        while idx < constant_pool_count - 1 {
            let tag = self.read_u8()?;
            // println!("tag: {:?}", tag);
            let const_ty = constant::u8_to_constant_type(tag)?;
            let constant = self.read_constant(&const_ty)?;
            println!(
                "tag({}): {:?}: {:?}",
                constant_pool.len(),
                const_ty,
                constant
            );

            constant_pool.push(constant);

            // https://docs.oracle.com/javase/specs/jvms/se8/html/jvms-4.html#jvms-4.4.5
            // > If a CONSTANT_Long_info or CONSTANT_Double_info structure is the item in the
            // > constant_pool table at index n, then the next usable item in the pool is located at
            // > index n+2. The constant_pool index n+1 must be valid but is considered unusable.
            match const_ty {
                ConstantType::Double | ConstantType::Long => {
                    constant_pool.push(Constant::None);
                    idx += 2;
                }
                _ => idx += 1,
            }
        }

        let access_flags = self.read_u16()?;
        println!("access_flags: {}", access_flags);

        let this_class = self.read_u16()?;
        println!("this_class: {}", this_class);

        let super_class = self.read_u16()?;
        println!("super_class: {}", super_class);

        let interfaces_count = self.read_u16()?;
        println!("interfaces_count: {}", interfaces_count);

        let mut interfaces = vec![];
        for _ in 0..interfaces_count {
            interfaces.push(self.read_constant_class_info()?);
        }
        println!("interfaces: {:?}", interfaces);

        let fields_count = self.read_u16()?;
        println!("fields_count: {}", fields_count);

        let mut fields = vec![];
        for _ in 0..fields_count {
            fields.push(self.read_field_info(&constant_pool)?);
        }
        println!("fields: {:?}", fields);

        let methods_count = self.read_u16()?;
        println!("methods_count: {}", methods_count);

        let mut methods = vec![];
        for _ in 0..methods_count {
            methods.push(self.read_method_info(&constant_pool)?);
        }
        // println!("methods: {:?}", methods);

        let attributes_count = self.read_u16()?;
        println!("attributes_count: {}", attributes_count);

        let mut attributes = vec![];
        for _ in 0..attributes_count {
            attributes.push(self.read_attribute_info(&constant_pool)?)
        }
        println!("attributes: {:?}", attributes);

        Some(ClassFile {
            magic: 0xCAFEBABE,
            minor_version,
            major_version,
            constant_pool_count,
            constant_pool,
            access_flags,
            this_class,
            super_class,
            interfaces_count,
            interfaces,
            fields_count,
            fields,
            methods_count,
            methods,
            attributes_count,
            attributes,
        })
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

// Fields

impl ClassFileReader {
    fn read_field_info(&mut self, constant_pool: &Vec<Constant>) -> Option<FieldInfo> {
        let access_flags = self.read_u16()?;
        let name_index = self.read_u16()?;
        let descriptor_index = self.read_u16()?;
        let attributes_count = self.read_u16()?;
        let mut attributes = vec![];
        for _ in 0..attributes_count {
            attributes.push(self.read_attribute_info(constant_pool)?)
        }
        Some(FieldInfo {
            access_flags,
            name_index,
            descriptor_index,
            attributes_count,
            attributes,
        })
    }
}

// Methods

impl ClassFileReader {
    fn read_method_info(&mut self, constant_pool: &Vec<Constant>) -> Option<MethodInfo> {
        let access_flags = self.read_u16()?;
        let name_index = self.read_u16()?;
        let descriptor_index = self.read_u16()?;
        let attributes_count = self.read_u16()?;
        let mut attributes = vec![];
        for _ in 0..attributes_count {
            attributes.push(self.read_attribute_info(constant_pool)?)
        }
        Some(MethodInfo {
            access_flags,
            name_index,
            descriptor_index,
            attributes_count,
            attributes,
        })
    }
}

// Attributes

impl ClassFileReader {
    fn read_attribute_info(&mut self, constant_pool: &Vec<Constant>) -> Option<AttributeInfo> {
        let attribute_name_index = self.read_u16()?;
        let attribute_length = self.read_u32()?;
        let name = constant_pool[attribute_name_index as usize].get_utf8()?;
        let info = match name.as_str() {
            "Code" => self.read_code_attribute(constant_pool)?,
            "LineNumberTable" => self.read_line_number_table_attribute()?,
            "SourceFile" => self.read_source_file_attribute()?,
            "StackMapTable" => self.read_stack_map_table_attribute()?,
            "Signature" => self.read_signature_attribute()?,
            "Exceptions" => self.read_exceptions_attribute()?,
            "Deprecated" => self.read_deprecated_attribute()?,
            "RuntimeVisibleAnnotations" => self.read_runtime_visible_annotations_attribute()?,
            "InnerClasses" => self.read_inner_classes_attribute()?,
            e => unimplemented!("{}", e),
        };
        Some(AttributeInfo {
            attribute_name_index,
            attribute_length,
            info,
        })
    }

    fn read_code_attribute(&mut self, constant_pool: &Vec<Constant>) -> Option<Attribute> {
        let max_stack = self.read_u16()?;
        let max_locals = self.read_u16()?;
        let code_length = self.read_u32()?;
        let mut code = vec![];
        for _ in 0..code_length {
            code.push(self.read_u8()?);
        }
        let exception_table_length = self.read_u16()?;
        let mut exception_table = vec![];
        for _ in 0..exception_table_length {
            exception_table.push(self.read_exception()?);
        }
        let attributes_count = self.read_u16()?;
        let mut attributes = vec![];
        for _ in 0..attributes_count {
            attributes.push(self.read_attribute_info(constant_pool)?)
        }
        Some(Attribute::Code {
            max_stack,
            max_locals,
            code_length,
            code,
            exception_table_length,
            exception_table,
            attributes_count,
            attributes,
        })
    }

    fn read_line_number_table_attribute(&mut self) -> Option<Attribute> {
        let line_number_table_length = self.read_u16()?;
        let mut line_number_table = vec![];
        for _ in 0..line_number_table_length {
            line_number_table.push(self.read_line_number()?)
        }
        Some(Attribute::LineNumberTable {
            line_number_table_length,
            line_number_table,
        })
    }

    fn read_source_file_attribute(&mut self) -> Option<Attribute> {
        let sourcefile_index = self.read_u16()?;
        Some(Attribute::SourceFile { sourcefile_index })
    }

    fn read_stack_map_table_attribute(&mut self) -> Option<Attribute> {
        let number_of_entries = self.read_u16()?;
        let mut entries = vec![];
        for _ in 0..number_of_entries {
            entries.push(self.read_stack_map_frame()?);
        }
        Some(Attribute::StackMapTable {
            number_of_entries,
            entries,
        })
    }

    fn read_signature_attribute(&mut self) -> Option<Attribute> {
        let signature_index = self.read_u16()?;
        Some(Attribute::Signature { signature_index })
    }

    fn read_exceptions_attribute(&mut self) -> Option<Attribute> {
        let number_of_exceptions = self.read_u16()?;
        let mut exception_index_table = vec![];
        for _ in 0..number_of_exceptions {
            exception_index_table.push(self.read_u16()?)
        }
        Some(Attribute::Exceptions {
            number_of_exceptions,
            exception_index_table,
        })
    }

    fn read_deprecated_attribute(&mut self) -> Option<Attribute> {
        Some(Attribute::Deprecated)
    }

    fn read_runtime_visible_annotations_attribute(&mut self) -> Option<Attribute> {
        let num_annotations = self.read_u16()?;
        let mut annotations = vec![];
        for _ in 0..num_annotations {
            annotations.push(self.read_annotation()?);
        }
        Some(Attribute::RuntimeVisibleAnnotations {
            num_annotations,
            annotations,
        })
    }

    fn read_inner_classes_attribute(&mut self) -> Option<Attribute> {
        let number_of_classes = self.read_u16()?;
        let mut classes = vec![];
        for _ in 0..number_of_classes {
            classes.push(self.read_classes()?)
        }
        Some(Attribute::InnerClasses {
            number_of_classes,
            classes,
        })
    }

    fn read_classes(&mut self) -> Option<InnerClassesBody> {
        let inner_class_info_index = self.read_u16()?;
        let outer_class_info_index = self.read_u16()?;
        let inner_name_index = self.read_u16()?;
        let inner_class_access_flags = self.read_u16()?;
        Some(InnerClassesBody {
            inner_class_info_index,
            outer_class_info_index,
            inner_name_index,
            inner_class_access_flags,
        })
    }

    fn read_annotation(&mut self) -> Option<Annotation> {
        let type_index = self.read_u16()?;
        let num_element_value_pairs = self.read_u16()?;
        let mut element_value_pairs = vec![];
        for _ in 0..num_element_value_pairs {
            element_value_pairs.push(self.read_element_value_pair()?);
        }
        Some(Annotation {
            type_index,
            num_element_value_pairs,
            element_value_pairs,
        })
    }

    fn read_element_value_pair(&mut self) -> Option<ElementValuePair> {
        let element_name_index = self.read_u16()?;
        println!("{}", element_name_index);
        let value = self.read_element_value()?;
        Some(ElementValuePair {
            element_name_index,
            value,
        })
    }

    fn read_element_value(&mut self) -> Option<ElementValue> {
        unimplemented!()
    }

    fn read_stack_map_frame(&mut self) -> Option<StackMapFrame> {
        let frame_type = self.read_u8()?;
        let body = match frame_type {
            0...63 => StackMapFrameBody::SameFrame,
            64...127 => {
                let stack = self.read_verification_type_info()?;
                StackMapFrameBody::SameLocals1StackItemFrame { stack }
            }
            252...254 => {
                let offset_delta = self.read_u16()?;
                let mut locals = vec![];
                for _ in 0..(frame_type - 251) {
                    locals.push(self.read_verification_type_info()?);
                }
                StackMapFrameBody::AppendFrame {
                    offset_delta,
                    locals,
                }
            }
            248...250 => {
                let offset_delta = self.read_u16()?;
                StackMapFrameBody::ChopFrame { offset_delta }
            }
            255 => {
                let offset_delta = self.read_u16()?;
                let number_of_locals = self.read_u16()?;
                let mut locals = vec![];
                for _ in 0..number_of_locals {
                    locals.push(self.read_verification_type_info()?);
                }
                let number_of_stack_items = self.read_u16()?;
                let mut stack = vec![];
                for _ in 0..number_of_stack_items {
                    stack.push(self.read_verification_type_info()?);
                }
                StackMapFrameBody::FullFrame {
                    offset_delta,
                    number_of_locals,
                    locals,
                    number_of_stack_items,
                    stack,
                }
            }
            // TODO: Implement all frame types
            e => unimplemented!("{}", e),
        };
        Some(StackMapFrame { frame_type, body })
    }

    fn read_verification_type_info(&mut self) -> Option<VerificationTypeInfo> {
        let tag = self.read_u8()?;
        match tag {
            1 => Some(VerificationTypeInfo::Integer),
            2 => Some(VerificationTypeInfo::Float),
            3 => Some(VerificationTypeInfo::Double),
            4 => Some(VerificationTypeInfo::Long),
            7 => {
                let cpool_index = self.read_u16()?;
                Some(VerificationTypeInfo::Object { cpool_index })
            }
            e => unimplemented!("verification type info {}", e),
        }
    }

    fn read_line_number(&mut self) -> Option<LineNumber> {
        let start_pc = self.read_u16()?;
        let line_number = self.read_u16()?;
        Some(LineNumber {
            start_pc,
            line_number,
        })
    }

    fn read_exception(&mut self) -> Option<Exception> {
        let start_pc = self.read_u16()?;
        let end_pc = self.read_u16()?;
        let handler_pc = self.read_u16()?;
        let catch_type = self.read_u16()?;
        Some(Exception {
            start_pc,
            end_pc,
            handler_pc,
            catch_type,
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
