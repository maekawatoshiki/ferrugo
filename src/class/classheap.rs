use super::class::Class;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ClassHeap {
    class_map: HashMap<String, Class>,
}


