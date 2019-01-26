use super::super::super::exec::vm::Inst;

#[derive(Debug, Clone)]
pub struct AttributeInfo {
    pub attribute_name_index: u16,
    pub attribute_length: u32,
    pub info: Attribute,
}

#[derive(Debug, Clone)]
pub struct CodeAttribute {
    pub max_stack: u16,
    pub max_locals: u16,
    pub code_length: u32,
    pub code: *mut Vec<u8>,
    pub exception_table_length: u16,
    pub exception_table: Vec<Exception>,
    pub attributes_count: u16,
    pub attributes: Vec<AttributeInfo>,
}

#[derive(Debug, Clone)]
pub enum Attribute {
    Code(CodeAttribute),
    LineNumberTable {
        line_number_table_length: u16,
        line_number_table: Vec<LineNumber>,
    },
    SourceFile {
        sourcefile_index: u16,
    },
    StackMapTable {
        number_of_entries: u16,
        entries: Vec<StackMapFrame>,
    },
    Signature {
        signature_index: u16,
    },
    Exceptions {
        number_of_exceptions: u16,
        exception_index_table: Vec<u16>,
    },
    Deprecated,
    RuntimeVisibleAnnotations {
        num_annotations: u16,
        annotations: Vec<Annotation>,
    },
    InnerClasses {
        number_of_classes: u16,
        classes: Vec<InnerClassesBody>,
    },
    ConstantValue {
        constantvalue_index: u16,
    },
}

#[derive(Debug, Clone)]
pub struct InnerClassesBody {
    pub inner_class_info_index: u16,
    pub outer_class_info_index: u16,
    pub inner_name_index: u16,
    pub inner_class_access_flags: u16,
}

#[derive(Debug, Clone)]
pub struct Exception {
    pub start_pc: u16,
    pub end_pc: u16,
    pub handler_pc: u16,
    pub catch_type: u16,
}

#[derive(Debug, Clone)]
pub struct LineNumber {
    pub start_pc: u16,
    pub line_number: u16,
}

#[derive(Debug, Clone)]
pub struct StackMapFrame {
    pub frame_type: u8,
    pub body: StackMapFrameBody,
}

#[derive(Debug, Clone)]
pub enum StackMapFrameBody {
    SameFrame,
    SameLocals1StackItemFrame {
        stack: VerificationTypeInfo,
    },
    AppendFrame {
        offset_delta: u16,
        locals: Vec<VerificationTypeInfo>,
    },
    ChopFrame {
        offset_delta: u16,
    },
    SameFrameExtended {
        offset_delta: u16,
    },
    FullFrame {
        offset_delta: u16,
        number_of_locals: u16,
        locals: Vec<VerificationTypeInfo>,
        number_of_stack_items: u16,
        stack: Vec<VerificationTypeInfo>,
    },
}

#[derive(Debug, Clone)]
pub enum VerificationTypeInfo {
    Top,
    Integer,
    Float,
    Long,
    Double,
    Null,
    UninitializedThis,
    Object { cpool_index: u16 },
    Uninitialized,
}

#[derive(Clone, Debug)]
pub struct Annotation {
    pub type_index: u16,
    pub num_element_value_pairs: u16,
    pub element_value_pairs: Vec<ElementValuePair>,
}

#[derive(Clone, Debug)]
pub struct ElementValuePair {
    pub element_name_index: u16,
    pub value: ElementValue,
}

#[derive(Clone, Debug)]
pub struct ElementValue {
    // TODO: Implement
}

impl CodeAttribute {
    pub fn read_u8_from_code(&self, start: usize) -> usize {
        let code = unsafe { &*self.code };
        code[start] as usize
    }

    pub fn read_u16_from_code(&self, start: usize) -> usize {
        let code = unsafe { &*self.code };
        ((code[start] as usize) << 8) + code[start + 1] as usize
    }

    pub fn dump_bytecode(&self) {
        let code = unsafe { &*self.code };
        let mut pc = 0;
        while pc < code.len() {
            let cur_code = code[pc];

            print!("    {:05}: ", pc);

            match cur_code {
                Inst::aconst_null => println!("aconst_null"),
                Inst::iconst_m1 => println!("iconst_m1"),
                Inst::iconst_0 => println!("iconst_0"),
                Inst::iconst_1 => println!("iconst_1"),
                Inst::iconst_2 => println!("iconst_2"),
                Inst::iconst_3 => println!("iconst_3"),
                Inst::iconst_4 => println!("iconst_4"),
                Inst::iconst_5 => println!("iconst_5"),
                Inst::dconst_0 => println!("dconst_0"),
                Inst::dconst_1 => println!("dconst_1"),
                Inst::bipush => println!("bipush"),
                Inst::sipush => println!("sipush"),
                Inst::ldc => println!("ldc"),
                Inst::ldc2_w => println!("ldc2_w"),
                Inst::iload => println!("iload"),
                Inst::dload => println!("dload"),
                Inst::aload_0 => println!("aload_0"),
                Inst::aload_1 => println!("aload_1"),
                Inst::aload_2 => println!("aload_2"),
                Inst::aload_3 => println!("aload_3"),
                Inst::istore => println!("istore"),
                Inst::istore_0 => println!("istore_0"),
                Inst::istore_1 => println!("istore_1"),
                Inst::istore_2 => println!("istore_2"),
                Inst::istore_3 => println!("istore_3"),
                Inst::aload => println!("aload"),
                Inst::iload_0 => println!("iload_0"),
                Inst::iload_1 => println!("iload_1"),
                Inst::iload_2 => println!("iload_2"),
                Inst::iload_3 => println!("iload_3"),
                Inst::dload_0 => println!("dload_0"),
                Inst::dload_1 => println!("dload_1"),
                Inst::dload_2 => println!("dload_2"),
                Inst::dload_3 => println!("dload_3"),
                Inst::iaload => println!("iaload"),
                Inst::daload => println!("daload"),
                Inst::aaload => println!("aaload"),
                Inst::dstore => println!("dstore"),
                Inst::astore => println!("astore"),
                Inst::dstore_0 => println!("dstore_0"),
                Inst::dstore_1 => println!("dstore_1"),
                Inst::dstore_2 => println!("dstore_2"),
                Inst::dstore_3 => println!("dstore_3"),
                Inst::astore_0 => println!("astore_0"),
                Inst::astore_1 => println!("astore_1"),
                Inst::astore_2 => println!("astore_2"),
                Inst::astore_3 => println!("astore_3"),
                Inst::iastore => println!("iastore"),
                Inst::dastore => println!("dastore"),
                Inst::aastore => println!("aastore"),
                Inst::pop => println!("pop"),
                Inst::pop2 => println!("pop2"),
                Inst::dup => println!("dup"),
                Inst::dup_x1 => println!("dup_x1"),
                Inst::dup2 => println!("dup2"),
                Inst::dup2_x1 => println!("dup2_x1"),
                Inst::iadd => println!("iadd"),
                Inst::dadd => println!("dadd"),
                Inst::isub => println!("isub"),
                Inst::dsub => println!("dsub"),
                Inst::imul => println!("imul"),
                Inst::dmul => println!("dmul"),
                Inst::idiv => println!("idiv"),
                Inst::ddiv => println!("ddiv"),
                Inst::irem => println!("irem"),
                Inst::dneg => println!("dneg"),
                Inst::ishl => println!("ishl"),
                Inst::ishr => println!("ishr"),
                Inst::iand => println!("iand"),
                Inst::ixor => println!("ixor"),
                Inst::iinc => println!("iinc"),
                Inst::i2d => println!("i2d"),
                Inst::d2i => println!("d2i"),
                Inst::i2s => println!("i2s"),
                Inst::dcmpl => println!("dcmpl"),
                Inst::dcmpg => println!("dcmpg"),
                Inst::ifeq => println!("ifeq"),
                Inst::ifne => println!("ifne"),
                Inst::iflt => println!("iflt"),
                Inst::ifge => println!("ifge"),
                Inst::ifle => println!("ifle"),
                Inst::if_icmpeq => println!("if_icmpeq"),
                Inst::if_icmpne => println!("if_icmpne"),
                Inst::if_icmpge => println!("if_icmpge"),
                Inst::if_icmpgt => println!("if_icmpgt"),
                Inst::if_icmplt => println!("if_icmplt"),
                Inst::if_acmpne => println!("if_acmpne"),
                Inst::goto => println!("goto"),
                Inst::ireturn => println!("ireturn"),
                Inst::dreturn => println!("dreturn"),
                Inst::areturn => println!("areturn"),
                Inst::return_ => println!("return_"),
                Inst::getstatic => println!("getstatic"),
                Inst::putstatic => println!("putstatic"),
                Inst::getfield => println!("getfield"),
                Inst::putfield => println!("putfield"),
                Inst::invokevirtual => println!("invokevirtual"),
                Inst::invokespecial => println!("invokespecial"),
                Inst::invokestatic => println!("invokestatic"),
                Inst::new => println!("new"),
                Inst::newarray => println!("newarray"),
                Inst::anewarray => println!("anewarray"),
                Inst::arraylength => println!("arraylength"),
                Inst::monitorenter => println!("monitorenter"),
                Inst::ifnull => println!("ifnull"),
                Inst::ifnonnull => println!("ifnonnull"),
                Inst::getfield_quick => println!("getfield_quick"),
                Inst::putfield_quick => println!("putfield_quick"),
                Inst::getfield2_quick => println!("getfield2_quick"),
                Inst::putfield2_quick => println!("putfield2_quick"),
                _ => unreachable!(),
            }

            pc += Inst::get_inst_size(cur_code);
        }
    }
}
