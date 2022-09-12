pub fn iter_blocks(block_size: u64, start: u64, size: u64) -> IterBlocks {
    IterBlocks {
        block_size,
        start,
        end: start + size,
        offset: 0,
    }
}

pub struct IterBlocks {
    block_size: u64,
    start: u64,
    end: u64,
    offset: u64,
}

impl IterBlocks {
    pub fn next(&mut self) -> Option<Block> {
        if self.start >= self.end {
            return None;
        }

        let block_num = self.start / self.block_size;
        let end = self.end.min((block_num + 1) * self.block_size);
        let block = Block {
            start: self.start,
            end,
            offset: self.offset,
            block_size: self.block_size,
        };
        self.offset += end - self.start;
        self.start = end;
        Some(block)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Block {
    pub start: u64,
    pub end: u64,
    pub offset: u64,
    pub block_size: u64,
}

impl Block {
    pub fn size(&self) -> u64 {
        self.end - self.start
    }

    pub fn num(&self) -> u64 {
        self.start / self.block_size
    }
}

#[test]
fn test_blocks() {
    fn collect(mut iterator: IterBlocks) -> Vec<Block> {
        let mut result = Vec::new();
        while let Some(block) = iterator.next() {
            result.push(block);
        }
        assert!(iterator.next().is_none());
        result
    }

    assert_eq!(
        collect(iter_blocks(10, 4, 4)),
        vec![
            Block { start: 4, end: 8, offset: 0, block_size: 10 },
        ],
    );
    assert_eq!(
        collect(iter_blocks(10, 24, 26)),
        vec![
            Block { start: 24, end: 30, offset: 0, block_size: 10 },
            Block { start: 30, end: 40, offset: 6, block_size: 10 },
            Block { start: 40, end: 50, offset: 16, block_size: 10 },
        ],
    );
    assert_eq!(
        collect(iter_blocks(10, 20, 26)),
        vec![
            Block { start: 20, end: 30, offset: 0, block_size: 10 },
            Block { start: 30, end: 40, offset: 10, block_size: 10 },
            Block { start: 40, end: 46, offset: 20, block_size: 10 },
        ],
    );
}
