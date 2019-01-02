use super::vm::Inst;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Block {
    pub code: Vec<Inst::Code>,
    pub start: usize,
    pub kind: BrKind,
    pub generated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum BrKind {
    ConditionalJmp { destinations: Vec<usize> },
    UnconditionalJmp { destination: usize },
    JmpRequired { destination: usize },
    BlockStart,
}

impl BrKind {
    pub fn get_conditional_jump_destinations(&self) -> &Vec<usize> {
        match self {
            BrKind::ConditionalJmp { destinations } => destinations,
            _ => panic!(),
        }
    }

    pub fn get_unconditional_jump_destination(&self) -> usize {
        match self {
            BrKind::UnconditionalJmp { destination } => *destination,
            BrKind::JmpRequired { destination } => *destination,
            _ => panic!(),
        }
    }
}

impl Block {
    pub fn code_end_position(&self) -> usize {
        self.start + self.code.len()
    }
}

#[derive(Debug, Clone)]
pub struct CFGMaker {}

impl CFGMaker {
    pub fn new() -> Self {
        CFGMaker {}
    }
}

impl CFGMaker {
    pub fn make(&mut self, code: &Vec<Inst::Code>, start: usize, end: usize) -> Vec<Block> {
        println!("{}~{}", start, end);
        let mut map = BTreeMap::new();
        let mut pc = start;

        while pc < end {
            let cur_code = code[pc];
            match cur_code {
                Inst::if_icmpne | Inst::if_icmpge => {
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
                }
                Inst::goto => {
                    let branch = ((code[pc + 1] as i16) << 8) + code[pc + 2] as i16;
                    let dst = (pc as isize + branch as isize) as usize;
                    map.insert(pc + 3 - 1, BrKind::UnconditionalJmp { destination: dst });
                    map.insert(dst, BrKind::BlockStart);
                }
                _ => {}
            }
            pc += Inst::get_inst_size(cur_code);
        }

        let mut cur = Some(start);
        let mut blocks = vec![];

        for (key, kind) in map {
            if kind != BrKind::BlockStart {
                if cur.is_some() && cur.unwrap() < end && cur.unwrap() < key {
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
                        generated: false,
                    });
                    cur = None;
                }
            } else {
                if cur.is_some() && cur.unwrap() < end && cur.unwrap() < key {
                    println!("[{}, {}]", cur.unwrap(), key - 1);
                    blocks.push(Block {
                        code: code[cur.unwrap()..key].to_vec(),
                        start: cur.unwrap(),
                        kind: BrKind::JmpRequired { destination: key },
                        generated: false,
                    });
                }
                cur = Some(key);
            }
        }
        if cur.is_some() && cur.unwrap() < end {
            println!("[{}, {}]", cur.unwrap(), end - 1);
            blocks.push(Block {
                code: code[cur.unwrap()..end].to_vec(),
                start: cur.unwrap(),
                kind: BrKind::BlockStart,
                generated: false,
            });
        }
        println!("{:?}", blocks);

        blocks
    }
}
