use super::config::NUM_BLOCKS_PER_TURN;
#[derive(Debug, Clone)]
pub enum DisplayPointStatus {
    Occupied,
    Unoccupied,
    Hovered { has_conflict: bool },
    Blast,
}

#[derive(Debug)]
pub struct BlockIndex {
    val: usize,
    num_left: usize,
}

impl BlockIndex {
    /// Construct a new block index.
    pub fn new(val: usize) -> Self {
        BlockIndex {
            val,
            num_left: NUM_BLOCKS_PER_TURN - 1,
        }
    }

    /// Retrieve the contained value.
    pub fn current(&self) -> usize {
        self.val
    }

    /// Retrieve the contained value and decrement the internal counter.
    pub fn place(&mut self) -> usize {
        let prev = self.val;
        self.num_left = if self.num_left == 0 {
            NUM_BLOCKS_PER_TURN - 1
        } else {
            self.num_left - 1
        };
        self.val = if self.val == 0 { 0 } else { self.val - 1 };
        prev
    }

    /// Get the next valid value.
    pub fn cycle(&mut self) -> &mut Self {
        if self.val == self.num_left {
            self.val = 0;
        } else {
            self.val += 1;
        }

        self
    }
}

impl Default for BlockIndex {
    fn default() -> Self {
        BlockIndex {
            val: 0,
            num_left: NUM_BLOCKS_PER_TURN - 1,
        }
    }
}
