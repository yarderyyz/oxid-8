use std::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

#[derive(Clone, Copy, PartialEq, Debug)]
struct BufferState {
    read_idx: usize,
    ready_idx: usize,
    write_idx: usize,
    dirty: bool,
}

impl BufferState {
    pub fn new() -> Self {
        Self {
            read_idx: 0,
            ready_idx: 1,
            write_idx: 2,
            dirty: false,
        }
    }
    pub fn decode(state: usize) -> Self {
        let read_idx = state & 0xF;
        let ready_idx = (state >> 4) & 0xF;
        let write_idx = (state >> 8) & 0xF;
        let dirty = (state >> 12) & 0xF == 1;
        Self {
            read_idx,
            ready_idx,
            write_idx,
            dirty,
        }
    }

    pub fn encode(&self) -> usize {
        let mut out: usize = 0;
        out |= self.read_idx;
        out |= self.ready_idx << 4;
        out |= self.write_idx << 8;
        out |= (self.dirty as usize) << 12;
        out
    }
}

pub struct TripleBuffer<T> {
    buffers: [UnsafeCell<T>; 3],
    encoded_state: AtomicUsize,
}

pub struct TripleBufferWriter<T> {
    buffer: Arc<TripleBuffer<T>>,
    borrowers: AtomicUsize,
}

impl<T> TripleBufferWriter<T> {
    pub fn write(&mut self) -> WriteHandle<T> {
        let state = self.buffer.state();
        let borrowers = self.add_handle();
        if borrowers > 0 {
            panic!("TripleBuffer can only have one active writer");
        }
        WriteHandle::new(self, &self.buffer.buffers[state.write_idx])
    }

    fn add_handle(&self) -> usize {
        self.borrowers.fetch_add(1, Ordering::Acquire)
    }

    fn drop_handle(&self) -> usize {
        self.buffer.swap_write();
        self.borrowers.fetch_sub(1, Ordering::Release)
    }
}

pub struct TripleBufferReader<T> {
    buffer: Arc<TripleBuffer<T>>,
    borrowers: AtomicUsize,
}

impl<T> TripleBufferReader<T> {
    pub fn read(&self) -> ReadHandle<T> {
        let borrowers = self.add_handle();
        let state = if borrowers == 0 {
            self.buffer.try_swap_read()
        } else {
            self.buffer.state()
        };
        ReadHandle::new(self, &self.buffer.buffers[state.read_idx])
    }

    fn add_handle(&self) -> usize {
        self.borrowers.fetch_add(1, Ordering::Acquire)
    }

    fn drop_handle(&self) -> usize {
        self.borrowers.fetch_sub(1, Ordering::Release)
    }
}

pub fn triple_buffer<T: Copy>(initial: T) -> (TripleBufferWriter<T>, TripleBufferReader<T>) {
    let buffer = Arc::new(TripleBuffer::new(initial));

    let writer = TripleBufferWriter {
        buffer: buffer.clone(),
        borrowers: 0.into(),
    };

    let reader = TripleBufferReader {
        buffer,
        borrowers: 0.into(),
    };

    (writer, reader)
}

pub struct ReadHandle<'a, T> {
    read_cell: &'a UnsafeCell<T>,
    parent: &'a TripleBufferReader<T>,
}

impl<'a, T> ReadHandle<'a, T> {
    pub fn new(parent: &'a TripleBufferReader<T>, read_cell: &'a UnsafeCell<T>) -> Self {
        Self { parent, read_cell }
    }
}

impl<T> Drop for ReadHandle<'_, T> {
    fn drop(&mut self) {
        self.parent.drop_handle();
    }
}

impl<T> Deref for ReadHandle<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // safety: the atomic state guarentee that the read_cell
        //         is only immutably borrowed.
        unsafe { &*self.read_cell.get() }
    }
}

pub struct WriteHandle<'a, T> {
    write_cell: &'a UnsafeCell<T>,
    parent: &'a TripleBufferWriter<T>,
}

impl<'a, T> WriteHandle<'a, T> {
    pub fn new(parent: &'a TripleBufferWriter<T>, write_cell: &'a UnsafeCell<T>) -> Self {
        Self { parent, write_cell }
    }
}

impl<T> Drop for WriteHandle<'_, T> {
    fn drop(&mut self) {
        self.parent.drop_handle();
    }
}

impl<T> Deref for WriteHandle<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        // safety: the atomic operations guarentee that the write_cell
        //         has no other active aliases
        unsafe { &*self.write_cell.get() }
    }
}
impl<T> DerefMut for WriteHandle<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // safety: the atomic state guarentee that the write_cell
        //         has no other active aliases
        unsafe { &mut *self.write_cell.get() }
    }
}

impl<T: Copy> TripleBuffer<T> {
    pub fn new(initial: T) -> Self {
        let encoded_state = BufferState::encode(&BufferState::new());
        Self {
            buffers: [
                UnsafeCell::new(initial),
                UnsafeCell::new(initial),
                UnsafeCell::new(initial),
            ],
            encoded_state: encoded_state.into(),
        }
    }
}

impl<T> TripleBuffer<T> {
    fn state(&self) -> BufferState {
        let current = self.encoded_state.load(Ordering::Relaxed);
        BufferState::decode(current)
    }

    fn try_swap_read(&self) -> BufferState {
        loop {
            let current = self.encoded_state.load(Ordering::Acquire);
            let state = BufferState::decode(current);
            if !state.dirty {
                return state;
            }

            let mut new_state = state;
            new_state.ready_idx = state.read_idx;
            new_state.read_idx = state.ready_idx;
            new_state.dirty = false;

            let new = new_state.encode();
            if self
                .encoded_state
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                return new_state;
            }
        }
    }

    fn swap_write(&self) -> BufferState {
        loop {
            let current = self.encoded_state.load(Ordering::Acquire);
            let state = BufferState::decode(current);

            let mut new_state = state;
            new_state.write_idx = state.ready_idx;
            new_state.ready_idx = state.write_idx;
            new_state.dirty = true;

            let new = new_state.encode();
            if self
                .encoded_state
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                return new_state;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_state_new() {
        let state = BufferState::new();
        assert_eq!(state.read_idx, 0);
        assert_eq!(state.ready_idx, 1);
        assert_eq!(state.write_idx, 2);
        assert!(!state.dirty);
    }

    #[test]
    fn test_buffer_state_equality() {
        let state1 = BufferState::new();
        let state2 = BufferState::new();
        assert_eq!(state1, state2);
    }

    #[test]
    fn test_buffer_state_encode_decode() {
        let mut state = BufferState::new();
        state.read_idx = 1;
        state.ready_idx = 2;
        state.write_idx = 0;
        state.dirty = true;
        let encoded_state = state.encode();
        let decoded_state = BufferState::decode(encoded_state);
        assert_eq!(state, decoded_state);
    }
}
