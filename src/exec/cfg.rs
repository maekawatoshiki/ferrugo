use super::vm::Inst;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Block {
    pub code: Vec<Inst::Code>,
    pub start: usize,
    pub kind: BrKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrKind {
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
    pub fn make(&mut self, code: &Vec<Inst::Code>) -> Vec<Block> {
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

        blocks
    }
}
