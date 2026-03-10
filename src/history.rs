#[derive(Clone, Debug)]
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    limit: usize,
}

impl<T: Clone> History<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(8),
        }
    }

    pub fn checkpoint(&mut self, current: &T) {
        self.undo.push(current.clone());
        if self.undo.len() > self.limit {
            let overflow = self.undo.len() - self.limit;
            self.undo.drain(0..overflow);
        }
        self.redo.clear();
    }

    pub fn undo(&mut self, current: &mut T) -> bool {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(current.clone());
            *current = previous;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self, current: &mut T) -> bool {
        if let Some(next) = self.redo.pop() {
            self.undo.push(current.clone());
            *current = next;
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
