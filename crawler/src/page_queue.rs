use std::collections::{HashSet, VecDeque};

use crate::page::Page;

pub struct PageQueue {
    queue: VecDeque<Page>,
    hashset: HashSet<Page>,
}

impl PageQueue {
    pub fn new() -> Self {
        let queue = VecDeque::new();
        let hashset = HashSet::new();

        PageQueue { queue, hashset }
    }
}
