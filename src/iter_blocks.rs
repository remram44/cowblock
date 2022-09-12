pub fn iter_blocks(block_size: u64, start: u64, size: u64) -> IterBlocks {
    IterBlocks {
        block_size,
        start,
        end: start + size,
        start_offset: 0,
    }
}

pub struct IterBlocks {
    block_size: u64,
    start: u64,
    end: u64,
    start_offset: u64,
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
            size: end - self.start,
            start_offset: self.start_offset,
            num: block_num,
        };
        self.start_offset += end - self.start;
        self.start = end;
        Some(block)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Block {
    pub start: u64,
    pub end: u64,
    pub size: u64,
    pub start_offset: u64,
    pub num: u64,
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
            Block { start: 4, end: 8, size: 4, start_offset: 0, num: 0 },
        ],
    );
    assert_eq!(
        collect(iter_blocks(10, 24, 26)),
        vec![
            Block { start: 24, end: 30, size: 6, start_offset: 0, num: 2 },
            Block { start: 30, end: 40, size: 10, start_offset: 6, num: 3 },
            Block { start: 40, end: 50, size: 10, start_offset: 16, num: 4 },
        ],
    );
    assert_eq!(
        collect(iter_blocks(10, 20, 26)),
        vec![
            Block { start: 20, end: 30, size: 10, start_offset: 0, num: 2 },
            Block { start: 30, end: 40, size: 10, start_offset: 10, num: 3 },
            Block { start: 40, end: 46, size: 6, start_offset: 20, num: 4 },
        ],
    );
}
