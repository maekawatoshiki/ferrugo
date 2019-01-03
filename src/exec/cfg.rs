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
        let mut map = BTreeMap::new();
        let mut pc = start;

        while pc < end {
            let cur_code = code[pc];
            match cur_code {
                // TODO: Add instructions
                Inst::if_icmpne | Inst::if_icmpge | Inst::if_icmpgt | Inst::ifne => {
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

        let mut start = Some(start);
        let mut blocks = vec![];

        for (key, kind) in map {
            match kind {
                BrKind::BlockStart => {
                    if start.is_some() && start.unwrap() < end && start.unwrap() < key {
                        dprintln!("cfg: range: [{}, {}]", start.unwrap(), key - 1);
                        blocks.push(Block {
                            code: code[start.unwrap()..key].to_vec(),
                            start: start.unwrap(),
                            kind: BrKind::JmpRequired { destination: key },
                            generated: false,
                        });
                    }
                    start = Some(key);
                }
                BrKind::ConditionalJmp { .. } | BrKind::UnconditionalJmp { .. }
                    if start.is_some() && start.unwrap() < end && start.unwrap() < key =>
                {
                    dprintln!(
                        "cfg: range: {}[{}, {}]",
                        match kind {
                            BrKind::ConditionalJmp { ref destinations } => {
                                format!("IF({:?}) ", destinations)
                            }
                            _ => "".to_string(),
                        },
                        start.unwrap(),
                        key
                    );
                    blocks.push(Block {
                        code: code[start.unwrap()..key + 1].to_vec(),
                        start: start.unwrap(),
                        kind,
                        generated: false,
                    });
                    start = None;
                }
                _ => {}
            }
        }

        if start.is_some() && start.unwrap() < end {
            dprintln!("cfg: range: [{}, {}]", start.unwrap(), end - 1);
            blocks.push(Block {
                code: code[start.unwrap()..end].to_vec(),
                start: start.unwrap(),
                kind: BrKind::BlockStart,
                generated: false,
            });
        }

        blocks
    }
}
