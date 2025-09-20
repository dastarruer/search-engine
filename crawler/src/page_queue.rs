use crate::page::Page;
use std::collections::{HashSet, VecDeque};

#[derive(Clone, Debug, PartialEq)]
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

    pub fn push(&mut self, page: Page) {
        self.queue.push_back(page.clone());
        self.hashset.insert(page);
    }

    pub fn pop(&mut self) -> Option<Page> {
        let page = self.queue.back();

        if let Some(page) = page {
            self.hashset.remove(page);
            self.queue.pop_back()
        } else {
            None
        }
    }

    pub fn contains_page(&self, page: &Page) -> bool {
        self.hashset.contains(page)
    }
}

impl PartialEq<VecDeque<Page>> for PageQueue {
    fn eq(&self, other: &VecDeque<Page>) -> bool {
        self.queue == *other
    }
}
