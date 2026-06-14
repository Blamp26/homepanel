use std::collections::VecDeque;

#[derive(Clone, Debug)]
pub struct ScrollbackBuffer {
    max_bytes: usize,
    data: VecDeque<u8>,
}

impl ScrollbackBuffer {
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes,
            data: VecDeque::new(),
        }
    }

    pub fn append(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.data.push_back(*byte);
        }
        while self.data.len() > self.max_bytes {
            self.data.pop_front();
        }
    }

    pub fn snapshot(&self) -> Vec<u8> {
        self.data.iter().copied().collect()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::ScrollbackBuffer;

    #[test]
    fn keeps_recent_bytes() {
        let mut buffer = ScrollbackBuffer::new(4);
        buffer.append(b"abc");
        buffer.append(b"def");
        assert_eq!(buffer.snapshot(), b"cdef");
    }

    #[test]
    fn clear_works() {
        let mut buffer = ScrollbackBuffer::new(16);
        buffer.append(b"hello");
        buffer.clear();
        assert_eq!(buffer.len(), 0);
    }
}
