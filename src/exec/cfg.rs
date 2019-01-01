use super::vm::Inst;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Block {
    code: Vec<Inst::Code>,
    start: usize,
    kind: BrKind,
}

#[derive(Clone, Debug, PartialEq)]
enum BrKind {
    ConditionalJmp { destinations: Vec<usize> },
    UnconditionalJmp { destination: usize },
    BlockStart,
}

#[derive(Debug, Clone)]
pub struct CFGMaker {}

impl CFGMaker {
    pub fn new() -> Self {
        CFGMaker {}
    }
}

impl CFGMaker {
    pub fn make(&mut self, code: &Vec<Inst::Code>) {
        let mut map = BTreeMap::new();
        let mut pc = 0;

        loop {
            if pc >= code.len() {
                break;
            }

            let cur_code = code[pc];

            match cur_code {
                Inst::if_icmpne => {
                    let branch = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    let dst = (pc as isize + branch as isize) as usize;
                    map.insert(
                        pc + 3 - 1,
                        BrKind::ConditionalJmp {
                            destinations: vec![dst, pc + 3],
                        },
                    );
                    map.insert(dst, BrKind::BlockStart);
                    map.insert(pc + 3, BrKind::BlockStart);
                    pc += 3;
                }
                Inst::goto => {
                    let branch = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    let dst = (pc as isize + branch as isize) as usize;
                    map.insert(pc + 3 - 1, BrKind::UnconditionalJmp { destination: dst });
                    map.insert(dst, BrKind::BlockStart);
                    pc += 3;
                }
                code => pc += Inst::get_inst_size(code),
            }
        }

        let mut cur = Some(0);
        let mut blocks = vec![];

        for (key, kind) in map {
            if kind != BrKind::BlockStart {
                if cur.is_some() {
                    println!(
                        "{}[{}, {}]",
                        match kind {
                            BrKind::ConditionalJmp { ref destinations } => {
                                format!("IF({:?}) ", destinations)
                            }
                            _ => "".to_string(),
                        },
                        cur.unwrap(),
                        key
                    );
                    blocks.push(Block {
                        code: code[cur.unwrap()..key + 1].to_vec(),
                        start: cur.unwrap(),
                        kind,
                    });
                    cur = None;
                }
            } else {
                if cur.is_some() {
                    println!("[{}, {}]", cur.unwrap(), key - 1);
                    blocks.push(Block {
                        code: code[cur.unwrap()..key].to_vec(),
                        start: cur.unwrap(),
                        kind: BrKind::UnconditionalJmp { destination: key },
                    });
                }
                cur = Some(key);
            }
        }
        if cur.is_some() {
            println!("[{}, {}]", cur.unwrap(), code.len() - 1);
            blocks.push(Block {
                code: code[cur.unwrap()..code.len()].to_vec(),
                start: cur.unwrap(),
                kind: BrKind::BlockStart,
            });
        }
        println!("{:?}", blocks);

        loop {
            if pc >= code.len() {
                break;
            }

            let cur_code = code[pc];

            match cur_code {
                Inst::iconst_m1
                | Inst::iconst_0
                | Inst::iconst_1
                | Inst::iconst_2
                | Inst::iconst_3
                | Inst::iconst_4
                | Inst::iconst_5 => {}
                Inst::dconst_0 | Inst::dconst_1 => {}
                Inst::dstore => {}
                Inst::astore => {}
                Inst::istore => {}
                Inst::istore_0 | Inst::istore_1 | Inst::istore_2 | Inst::istore_3 => {}
                Inst::iload_0 | Inst::iload_1 | Inst::iload_2 | Inst::iload_3 => {}
                Inst::dload_0 | Inst::dload_1 | Inst::dload_2 | Inst::dload_3 => {}
                Inst::iaload => {}
                Inst::aaload => {}
                Inst::sipush => {}
                Inst::ldc => {}
                Inst::ldc2_w => {}
                Inst::aload => {}
                Inst::dload => {}
                Inst::iload => {}
                Inst::aload_0 | Inst::aload_1 | Inst::aload_2 | Inst::aload_3 => {}
                Inst::dstore_0 | Inst::dstore_1 | Inst::dstore_2 | Inst::dstore_3 => {}
                Inst::astore_0 | Inst::astore_1 | Inst::astore_2 | Inst::astore_3 => {}
                Inst::iastore => {}
                Inst::aastore => {}
                Inst::bipush => {}
                Inst::iadd => {}
                Inst::dadd => {}
                Inst::isub => {}
                Inst::dsub => {}
                Inst::imul => {}
                Inst::dmul => {}
                Inst::ddiv => {}
                Inst::irem => {}
                Inst::dneg => {}
                Inst::iinc => {}
                Inst::i2d => {}
                Inst::i2s => {}
                Inst::invokestatic => {}
                Inst::invokespecial => {}
                Inst::invokevirtual => {}
                Inst::new => {}
                Inst::newarray => {}
                Inst::anewarray => {}
                Inst::pop | Inst::pop2 => {}
                Inst::dup => {}
                Inst::goto => {}
                Inst::dcmpl => {}
                Inst::ifeq => {}
                Inst::ifne => {}
                Inst::if_icmpne => {}
                Inst::if_icmpge => {}
                Inst::if_icmpgt => {}
                Inst::ireturn => {}
                Inst::dreturn => {}
                Inst::return_ => {}
                Inst::getstatic => {}
                Inst::putstatic => {}
                Inst::getfield => {}
                Inst::putfield => {}
                Inst::monitorenter => {}
                e => unimplemented!("{}", e),
            }
        }
    }
}
