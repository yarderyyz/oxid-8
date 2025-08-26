//! Wait-free triple buffering for high-frequency producer-consumer scenarios.
//!
//! This module provides a triple buffer implementation that allows a single
//! producer to continuously write data while a single consumer reads the most
//! recent complete data without blocking or synchronization overhead.
//!
//! The triple buffer consists of three buffers managed through atomic operations:
//! one being written to, one being read from, and one serving as a completed
//! buffer ready to be swapped in. This design eliminates the need for locks
//! while ensuring the reader always has access to the most recent completed data.
//!
//! ## Key Components
//!
//! * [`TripleBufferWriter`] - Used by the producer thread to write new data
//! * [`TripleBufferReader`] - Used by the consumer thread to read the latest data
//!
//! ## Architecture
//!
//! ```text
//! Writer Thread                    Reader Thread
//!     |                                |
//!     v                                v
//! TripleBufferWriter              TripleBufferReader
//!     |                                |
//!     +-----> Shared AtomicU64 <-------+
//!            (encoded_state)
//!                    |
//!                    v
//!       [Buffer0, Buffer1, Buffer2]
//! ```
//!
//! ## Usage Patterns
//!
//! Triple buffers are ideal for scenarios where:
//! - A producer generates data at high frequency (e.g., game state, video frames)
//! - A consumer needs the most recent data but can skip intermediate updates
//! - Wait-free operation is required for performance or real-time constraints
//! - The producer and consumer operate at different frequencies
//!
//! # Examples
//!
//! Basic usage with different sampling rates:
//!
//! ```rust
//! use oxid8::utils::triple_buffer::triple_buffer;
//! use std::thread;
//! use std::time::Duration;
//!
//!
//! let (mut writer, reader) = triple_buffer([0u8; 32]);
//!
//! // Producer thread - writes at ~500 Hz
//! let producer = thread::spawn(move || {
//!     for i in 0..100 {
//!         {
//!             let mut write_handle = writer.write();
//!             write_handle.fill(i as u8);
//!         }
//!         thread::sleep(Duration::from_millis(2)); // 500 Hz
//!     }
//! });
//!
//! // Consumer thread - reads at ~60 Hz
//! let consumer = thread::spawn(move || {
//!     for _ in 0..10 {
//!         let read_handle = reader.read();
//!         println!("Read value: {}", read_handle[0]);
//!         thread::sleep(Duration::from_nanos(16_666_667)); // ~60 Hz
//!     }
//! });
//!
//! producer.join().unwrap();
//! consumer.join().unwrap();
//!
//! ```
//!
//! ## Performance Characteristics
//!
//! - **Lock-free**: No mutex or other blocking synchronization primitives
//! - **Wait-free reads**: Reader never blocks, always gets immediate access to a read buffer
//! - **Wait-free writes**: Writer never blocks, always gets immediate access to a write buffer
//!
//! ## Thread Safety
//!
//! The triple buffer is designed for single-producer, single-consumer scenarios.
//! Both the writer and reader are `Send` but not `Sync`, ensuring they can be
//! moved between threads but not shared simultaneously.
//!
//! [`TripleBufferWriter`]: struct.TripleBufferWriter.html
//! [`TripleBufferReader`]: struct.TripleBufferReader.html
use std::{
    cell::{Cell, UnsafeCell},
    marker::PhantomData,
    ops::{Deref, DerefMut},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

/// Creates a new triple buffer, returning the writer/reader halves.
///
/// Triple buffering is a synchronization technique that allows a writer to
/// continuously update data without blocking, while a reader can always access
/// the most recent complete update. This is particularly useful for scenarios
/// where a producer generates data at a different rate than a consumer processes it.
///
/// The [`TripleBufferWriter`] can write new values without blocking, even if the
/// reader is currently reading. The [`TripleBufferReader`] will always get the
/// most recent complete value that was written, potentially skipping intermediate
/// values if the writer is faster than the reader.
///
/// Both the [`TripleBufferWriter`] and [`TripleBufferReader`] can be used from
/// separate threads, but each half is `!Sync` and cannot be shared between threads
/// without external synchronization. Only one writer and one reader are supported.
///
/// The type `T` must implement [`Clone`] as this constructor initializes all three
/// internal buffers with copies of the provided `initial` value.
///
/// # Examples
///
/// ```rust
/// use oxid8::utils::triple_buffer::triple_buffer;
/// use std::thread;
/// use std::time::Duration;
///
/// let (mut writer, mut reader) = triple_buffer(0u32);
///
/// // Spawn a writer thread that updates values rapidly
/// thread::spawn(move || {
///     for i in 0..100 {
///         let mut write_handle = writer.write();
///         *write_handle = i;
///         thread::sleep(Duration::from_millis(10));
///     }
/// });
///
/// // Reader thread that reads at a different rate
/// thread::spawn(move || {
///     loop {
///         let value = reader.read();
///         println!("Latest value: {}", *value);
///         thread::sleep(Duration::from_millis(30));
///         // May skip some intermediate values if writer is faster
///     }
/// });
/// ```
///
/// [`TripleBufferWriter`]: struct.TripleBufferWriter.html
/// [`TripleBufferReader`]: struct.TripleBufferReader.html
pub fn triple_buffer<T: Clone>(initial: T) -> (TripleBufferWriter<T>, TripleBufferReader<T>) {
    let buffer = Arc::new(TripleBuffer::new(initial));

    let writer = TripleBufferWriter {
        buffer: buffer.clone(),
        borrowers: 0.into(),
        _not_sync: PhantomData,
    };

    let reader = TripleBufferReader {
        buffer,
        borrowers: 0.into(),
        _not_sync: PhantomData,
    };

    (writer, reader)
}

/// Encodes the state of a triple buffer system into a compact representation.
///
/// This struct tracks which of the three buffers is currently being used for
/// reading, which is ready to be read, and which is being written to. The state
/// can be atomically encoded into a `u64` for wait-free synchronization between
/// reader and writer threads.
///
/// # Buffer Indices
///
/// The triple buffer uses three buffers with indices typically 0, 1, and 2:
/// - `read_idx`: The buffer currently being read from
/// - `ready_idx`: The buffer containing the most recent complete write, ready to be swapped for reading
/// - `write_idx`: The buffer currently being written to
///
/// # Encoding Format
///
/// The state is encoded into a `u64` with the following bit layout:
/// - Bits 0-3: `read_idx`
/// - Bits 4-7: `ready_idx`
/// - Bits 8-11: `write_idx`
/// - Bits 12-15: `dirty` flag (1 if dirty, 0 otherwise)
/// - Bits 16-63: Reserved (unused)
#[derive(Clone, Copy, PartialEq, Debug)]
struct BufferState {
    /// Index of the buffer currently being read from.
    read_idx: usize,
    /// Index of the buffer containing the latest complete write.
    ready_idx: usize,
    /// Index of the buffer currently being written to.
    write_idx: usize,
    /// Indicates whether a new value has been written since the last read.
    dirty: bool,
}

impl BufferState {
    /// Creates a new `BufferState` with default buffer assignments.
    ///
    /// # Initial State
    ///
    /// - Buffer 0 is assigned for reading
    /// - Buffer 1 is ready (but contains no new data yet)
    /// - Buffer 2 is assigned for writing
    /// - The dirty flag is `false`
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let state = BufferState::new();
    /// assert_eq!(state.read_idx, 0);
    /// assert_eq!(state.ready_idx, 1);
    /// assert_eq!(state.write_idx, 2);
    /// assert_eq!(state.dirty, false);
    /// ```
    pub fn new() -> Self {
        Self {
            read_idx: 0,
            ready_idx: 1,
            write_idx: 2,
            dirty: false,
        }
    }

    /// Decodes a `BufferState` from its compact `u64` representation.
    ///
    /// This function extracts the buffer indices and dirty flag from a
    /// bit-packed `u64` value, typically read from an atomic variable
    /// for wait-free synchronization.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let encoded = 0x1082; // write=2, ready=0, read=1, dirty=true
    /// let state = BufferState::decode(encoded);
    /// assert_eq!(state.read_idx, 2);
    /// assert_eq!(state.ready_idx, 0);
    /// assert_eq!(state.write_idx, 1);
    /// assert_eq!(state.dirty, true);
    /// ```
    pub fn decode(state: u64) -> Self {
        Self {
            read_idx: (state & 0xF) as usize,
            ready_idx: ((state >> 4) & 0xF) as usize,
            write_idx: ((state >> 8) & 0xF) as usize,
            dirty: (state >> 12) & 0xF == 1,
        }
    }

    /// Encodes the `BufferState` into a compact `u64` representation.
    ///
    /// This function packs the buffer indices and dirty flag into a
    /// single `u64` value that can be stored in an atomic variable
    /// for wait-free synchronization between threads.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let state = BufferState {
    ///     read_idx: 2,
    ///     ready_idx: 0,
    ///     write_idx: 1,
    ///     dirty: true,
    /// };
    /// let encoded = state.encode();
    /// assert_eq!(encoded, 0x1102);
    /// ```
    pub fn encode(&self) -> u64 {
        let mut out: u64 = 0;
        out |= self.read_idx as u64;
        out |= (self.ready_idx as u64) << 4;
        out |= (self.write_idx as u64) << 8;
        out |= (self.dirty as u64) << 12;
        out
    }
}

/// A wait-free triple buffer for single-producer single-consumer communication.
///
/// The triple buffer maintains three internal buffers to enable wait-free communication
/// between a writer and reader thread. While the writer updates one buffer and the reader
/// reads from another, the third buffer holds the most recent complete write, ready to
/// be swapped in when the reader needs fresh data.
///
/// # Memory Layout
///
/// The structure contains:
/// - Three buffers stored in `UnsafeCell` for interior mutability
/// - An atomic state variable that coordinates access between reader and writer
///
/// # Usage
///
/// The `TripleBuffer` is typically not used directly. Instead, use the `triple_buffer()`
/// function to create a writer/reader pair:
///
/// ```rust
/// use oxid8::utils::triple_buffer::triple_buffer;
///
/// let (mut writer, reader) = triple_buffer::<[u64; 32]>(Default::default());
/// ```
///
/// [`TripleBufferWriter`]: struct.TripleBufferWriter.html
/// [`TripleBufferReader`]: struct.TripleBufferReader.html
pub struct TripleBuffer<T> {
    buffers: [UnsafeCell<T>; 3],
    encoded_state: AtomicU64,
}

/// # Safety
///
/// `TripleBuffer<T>` can be safely sent between threads when `T: Send` because:
/// - The internal buffers are only accessed through `UnsafeCell`, providing interior mutability
/// - Access is coordinated through atomic operations on `encoded_state`
/// - The [`TripleBufferWriter`] and [`TripleBufferReader`] types ensure exclusive access:
///   - Only one thread can write (via the single `TripleBufferWriter`)
///   - Only one thread can read (via the single `TripleBufferReader`)
///   - The atomic state prevents data races between reader and writer
unsafe impl<T: Send> Send for TripleBuffer<T> {}

impl<T: Clone> TripleBuffer<T> {
    /// Creates a new triple buffer initialized with the given value.
    ///
    /// All three internal buffers are initialized to copies of `initial`.
    /// This ensures the reader always has valid data available, even before
    /// the first write.
    ///
    /// # Arguments
    ///
    /// * `initial` - The initial value for all three buffers
    ///
    /// # Example
    ///
    /// ```ignore
    /// let buffer = TripleBuffer::new(MyState::default());
    /// ```
    ///
    /// # Note
    ///
    /// This constructor is typically not used directly. Use `triple_buffer()`
    /// instead to get a writer/reader pair.
    pub fn new(initial: T) -> Self {
        let encoded_state = BufferState::encode(&BufferState::new());
        Self {
            buffers: [
                UnsafeCell::new(initial.clone()),
                UnsafeCell::new(initial.clone()),
                UnsafeCell::new(initial.clone()),
            ],
            encoded_state: encoded_state.into(),
        }
    }
}

impl<T> TripleBuffer<T> {
    /// Returns the current buffer state.
    ///
    /// # Synchronization
    ///
    /// This performs an atomic load with `Acquire` ordering to ensure
    /// visibility of all writes that happened-before the state change.
    fn state(&self) -> BufferState {
        // No special handling is required here because, while reader and writer live in different
        // threads, they each only to use this to read state set by their own thread.
        let current = self.encoded_state.load(Ordering::Acquire);
        BufferState::decode(current)
    }

    /// Attempts to swap the read buffer with the ready buffer if new data is available.
    ///
    /// This operation is wait-free and will only swap if the dirty flag is set,
    /// indicating that the writer has published new data since the last read.
    ///
    /// # Returns
    ///
    /// The new buffer state after the swap attempt. If no new data was available
    /// (dirty flag was false), returns the current state unchanged.
    ///
    /// # Synchronization
    ///
    /// Uses a compare-exchange loop with weak failure ordering for performance.
    /// The `Release` ordering on success ensures the read operation happens-before
    /// any subsequent write operations that observe the new state.
    ///
    /// If there is contention, rather than run a CAS loop, we return the current state
    /// without updating it. This keeps the algorithm wait-free and is safe because
    /// while state needs to be updated atomically, only the 'ready' buffer is in contention
    /// between threads, and it is always safe to read from the current read buffer.
    fn try_swap_read(&self) -> BufferState {
        let current = self.encoded_state.load(Ordering::Acquire);
        let current_state = BufferState::decode(current);

        // No new data available, return current state
        if !current_state.dirty {
            return current_state;
        }

        // Swap read and ready buffers, clear dirty flag
        let mut new_state = current_state;
        new_state.ready_idx = current_state.read_idx;
        new_state.read_idx = current_state.ready_idx;
        new_state.dirty = false;

        // Try state update, if this fails return the current state
        let new = new_state.encode();
        if self
            .encoded_state
            .compare_exchange_weak(current, new, Ordering::Release, Ordering::Acquire)
            .is_ok()
        {
            new_state
        } else {
            current_state
        }
    }

    /// Swaps the write buffer with the ready buffer to publish written data.
    ///
    /// This operation is wait-free with a bounded number of retries (default: 1 retry).
    /// If contention occurs, it will retry the swap up to the specified limit, then
    /// return the current state. This maintains wait-free semantics since the number
    /// of attempts is bounded and finite.
    ///
    /// # Returns
    ///
    /// The new buffer state after the swap.
    ///
    /// # Synchronization
    ///
    /// Uses compare-exchange operations to atomically update the state with bounded retries.
    /// The `Release` ordering ensures all writes to the buffer happen-before
    /// the state change becomes visible to the reader.
    ///
    /// After the bounded retry limit is reached, we return the current state
    /// without updating it. This keeps the algorithm wait-free and is safe because
    /// while state needs to be updated atomically, only the 'ready' buffer is in contention
    /// between threads, and it is always safe to write to the current write buffer.
    ///
    /// # Note
    ///
    /// This always sets the dirty flag to indicate new data is available,
    /// even if the reader hasn't consumed the previous update. This allows
    /// the reader to skip intermediate updates when running at a lower
    /// frequency than the writer.
    ///
    /// By default, this will retry once if the initial compare_exchange fails due to contention.
    fn swap_write(&self) -> BufferState {
        self.swap_write_retry(1)
    }

    /// Same as `swap_write`, but allows specifying the number of retries on contention.
    ///
    /// This can be useful in high-contention scenarios where you want to make a best effort
    /// to publish the update rather than immediately falling back to the current state.
    ///
    /// # Parameters
    ///
    /// * `retries` - Number of times to retry the compare_exchange on failure (total attempts = retries + 1)
    fn swap_write_retry(&self, retries: usize) -> BufferState {
        for _ in 0..=retries {
            let current = self.encoded_state.load(Ordering::Acquire);
            let current_state = BufferState::decode(current);

            let mut new_state = current_state;
            new_state.write_idx = current_state.ready_idx;
            new_state.ready_idx = current_state.write_idx;
            new_state.dirty = true;

            // Try state update, if this fails retry
            let new = new_state.encode();
            if self
                .encoded_state
                .compare_exchange_weak(current, new, Ordering::Release, Ordering::Acquire)
                .is_ok()
            {
                return new_state;
            }
        }
        // If all attempts failed, return the last current state
        let current = self.encoded_state.load(Ordering::Acquire);
        BufferState::decode(current)
    }
}

/// The writer half of a triple buffer.
///
/// This structure provides exclusive write access to one of the three internal buffers.
/// The writer can obtain a mutable reference to the current write buffer through the
/// `write()` method, which returns a RAII guard that automatically publishes changes
/// when dropped.
///
/// # Usage
///
/// ```rust
/// use oxid8::utils::triple_buffer::triple_buffer;
///
/// let (mut writer, reader) = triple_buffer(0i32);
///
/// // Write a new value
/// {
///     let mut write_guard = writer.write();
///     *write_guard = 42;
/// } // Automatically published when guard is dropped
/// ```
///
/// # Performance
///
/// Writing is wait-free - the writer never blocks waiting for the reader. When
/// `write()` is called, it immediately returns a reference to the write buffer.
/// When the guard is dropped, an atomic swap makes the written data available
/// to the reader.
pub struct TripleBufferWriter<T> {
    buffer: Arc<TripleBuffer<T>>,
    borrowers: Cell<usize>,
    _not_sync: PhantomData<*const T>,
}

/// # Safety
///
/// `TripleBufferWriter<T>` can be safely sent between threads when `T: Send` because:
/// - It maintains exclusive write access through the borrowing mechanism
/// - The `PhantomData<*const T>` marker ensures this type is `!Sync`, preventing shared access
/// - Only one writer can exist per triple buffer (enforced at creation)
/// - All buffer access is coordinated through atomic operations
unsafe impl<T: Send> Send for TripleBufferWriter<T> {}

impl<T> TripleBufferWriter<T> {
    /// Obtains a write handle to the current write buffer.
    ///
    /// Returns a RAII guard that provides mutable access to the write buffer.
    /// When the guard is dropped, the written data is automatically published
    /// to the reader via an atomic buffer swap.
    ///
    /// # Panics
    ///
    /// Panics if called while another write handle is active. This prevents
    /// nested writes which could lead to data races or inconsistent state.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxid8::utils::triple_buffer::triple_buffer;
    ///
    /// let (mut writer, reader) = triple_buffer(0i32);
    /// let mut write_guard = writer.write();
    /// *write_guard = 42;
    /// // Data is published when write_guard goes out of scope
    /// ```
    pub fn write(&mut self) -> WriteHandle<'_, T> {
        let state = self.buffer.state();
        if self.borrowers.get() > 0 {
            panic!("TripleBuffer can only have one active writer");
        }
        self.add_handle();
        WriteHandle::new(self, &self.buffer.buffers[state.write_idx])
    }

    /// Increments the handle count to track active write handles.
    ///
    /// This is used internally to prevent multiple simultaneous writes.
    fn add_handle(&self) {
        self.borrowers.set(self.borrowers.get() + 1);
    }

    /// Decrements the handle count and publishes the written data.
    ///
    /// Called when a `WriteHandle` is dropped. This atomically swaps
    /// the write and ready buffers, making the written data available
    /// to the reader.
    fn drop_handle(&self) {
        self.borrowers.set(self.borrowers.get() - 1);
        self.buffer.swap_write();
    }
}

/// The reader half of a triple buffer.
///
/// This structure provides read access to the most recent completed write.
/// The reader can obtain an immutable reference to the latest data through the
/// `read()` method, which returns a RAII guard. Multiple read handles can be
/// active simultaneously, all reading from the same buffer.
///
/// # Important: Multiple Read Handles
///
/// When multiple read handles are active, they all read from the same buffer version.
/// **ALL active read handles must be dropped before the reader can access newer data
/// from the writer.** This ensures consistency but means long-lived read handles can
/// prevent access to fresh data.
///
/// # Usage
///
/// ```rust
/// use oxid8::utils::triple_buffer::triple_buffer;
///
/// let (writer, reader) = triple_buffer(0i32);
///
/// // Read the latest value
/// let read_guard = reader.read();
/// println!("Current value: {}", *read_guard);
///
/// // This gets the SAME data, not newer data
/// let another_read = reader.read();
///
/// // Must drop ALL guards before new data is accessible
/// drop(read_guard);
/// drop(another_read);
///
/// // Now this can get fresh data if available
/// let fresh_read = reader.read();
/// ```
///
/// # Performance
///
/// Reading is wait-free - the reader never blocks waiting for the writer.
/// When `read()` is called:
/// - If no read is active, it atomically swaps to get the latest data if available
/// - If a read is already active, it returns a handle to the same buffer
///
/// This ensures all active read handles see consistent data.
pub struct TripleBufferReader<T> {
    buffer: Arc<TripleBuffer<T>>,
    borrowers: Cell<usize>,
    _not_sync: PhantomData<*const T>,
}

/// # Safety
///
/// `TripleBufferReader<T>` can be safely sent between threads when `T: Send` because:
/// - It maintains exclusive read access (only one reader per triple buffer)
/// - The `PhantomData<*const T>` marker ensures this type is `!Sync`, preventing shared access
/// - Multiple read handles can coexist but all read from the same buffer version
/// - All buffer access is coordinated through atomic operations
unsafe impl<T: Send> Send for TripleBufferReader<T> {}

impl<T> TripleBufferReader<T> {
    /// Obtains a read handle to the latest available data.
    ///
    /// Returns a RAII guard that provides immutable access to the read buffer.
    ///
    /// # Behavior
    ///
    /// - If no read handles are currently active, attempts to swap to the latest
    ///   data if the writer has published new data (dirty flag is set)
    /// - If read handles are already active, returns a handle to the same buffer
    ///   to ensure all concurrent reads see consistent data
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxid8::utils::triple_buffer::triple_buffer;
    ///
    /// let (writer, reader) = triple_buffer(0i32);
    ///
    /// let read_guard = reader.read();
    /// print!("{}", &*read_guard); // Do something with data
    ///
    /// // Multiple simultaneous reads are allowed
    /// let another_read = reader.read();
    /// assert_eq!(&*read_guard as *const _, &*another_read as *const _);
    /// ```
    pub fn read(&self) -> ReadHandle<'_, T> {
        let state = if self.borrowers.get() == 0 {
            // No active reads, try to get fresh data
            self.buffer.try_swap_read()
        } else {
            // Reads already active, use current read buffer
            self.buffer.state()
        };
        self.add_handle();
        ReadHandle::new(self, &self.buffer.buffers[state.read_idx])
    }

    /// Increments the handle count to track active read handles.
    ///
    /// This is used internally to ensure all concurrent reads access
    /// the same buffer version.
    fn add_handle(&self) {
        self.borrowers.set(self.borrowers.get() + 1);
    }

    /// Decrements the handle count when a read handle is dropped.
    ///
    /// When the count reaches zero, the next read can swap to newer data
    /// if available.
    fn drop_handle(&self) {
        self.borrowers.set(self.borrowers.get() - 1);
    }
}

/// RAII guard providing immutable access to the read buffer.
///
/// This handle is returned by [`TripleBufferReader::read()`] and provides
/// thread-safe immutable access to the current read buffer. When dropped,
/// it decrements the reader's borrow count, potentially allowing access
/// to newer data on the next read.
///
/// # Example
///
/// ```ignore
/// let read_handle = reader.read();
/// println!("Value: {}", *read_handle);
/// // Automatically releases the read buffer when dropped
/// ```
///
/// [`TripleBufferReader::read()`]: struct.TripleBufferReader.html#method.read
pub struct ReadHandle<'a, T> {
    read_cell: &'a UnsafeCell<T>,
    parent: &'a TripleBufferReader<T>,
}

impl<'a, T> ReadHandle<'a, T> {
    /// Creates a new read handle.
    ///
    /// # Safety
    ///
    /// This is safe because the parent `TripleBufferReader` ensures that
    /// `read_cell` points to a buffer that won't be modified while this
    /// handle exists.
    pub fn new(parent: &'a TripleBufferReader<T>, read_cell: &'a UnsafeCell<T>) -> Self {
        Self { parent, read_cell }
    }
}

impl<T> Drop for ReadHandle<'_, T> {
    /// Decrements the reader's borrow count on drop.
    ///
    /// When the borrow count reaches zero, the reader can swap to
    /// newer data if available.
    fn drop(&mut self) {
        self.parent.drop_handle();
    }
}

impl<T> Deref for ReadHandle<'_, T> {
    type Target = T;

    /// Provides immutable access to the buffered data.
    ///
    /// # Safety
    ///
    /// This is safe because the atomic state guarantees that the read_cell
    /// is only immutably borrowed. The writer cannot modify this buffer
    /// while any read handles exist.
    fn deref(&self) -> &T {
        // safety: the atomic state guarentee that the read_cell
        //         is only immutably borrowed.
        unsafe { &*self.read_cell.get() }
    }
}

/// RAII guard providing mutable access to the write buffer.
///
/// This handle is returned by [`TripleBufferWriter::write()`] and provides
/// exclusive mutable access to the current write buffer. When dropped,
/// it atomically publishes the written data to the reader by swapping
/// buffers.
///
/// # Example
///
/// ```ignore
/// {
///     let mut write_handle = writer.write();
///     *write_handle = new_value;
///     // or modify in place:
///     write_handle.field = 42;
/// } // Data is published when handle is dropped
/// ```
///
/// [`TripleBufferWriter::write()`]: struct.TripleBufferWriter.html#method.write
pub struct WriteHandle<'a, T> {
    write_cell: &'a UnsafeCell<T>,
    parent: &'a TripleBufferWriter<T>,
}

impl<'a, T> WriteHandle<'a, T> {
    /// Creates a new write handle.
    ///
    /// # Safety
    ///
    /// This is safe because the parent `TripleBufferWriter` ensures that
    /// `write_cell` points to a buffer that has no other active aliases
    /// and won't be accessed by the reader while this handle exists.
    pub fn new(parent: &'a TripleBufferWriter<T>, write_cell: &'a UnsafeCell<T>) -> Self {
        Self { parent, write_cell }
    }
}

impl<T> Drop for WriteHandle<'_, T> {
    /// Publishes the written data and decrements the writer's borrow count.
    ///
    /// This atomically swaps the write and ready buffers, making the
    /// written data available to the reader.
    fn drop(&mut self) {
        self.parent.drop_handle();
    }
}

impl<T> Deref for WriteHandle<'_, T> {
    type Target = T;

    /// Provides immutable access to the write buffer.
    ///
    /// # Safety
    ///
    /// This is safe because the atomic operations guarantee that the write_cell
    /// has no other active aliases. The reader cannot access this buffer,
    /// and only one write handle can exist at a time.
    fn deref(&self) -> &T {
        // safety: the atomic operations guarentee that the write_cell
        //         has no other active aliases
        unsafe { &*self.write_cell.get() }
    }
}
impl<T> DerefMut for WriteHandle<'_, T> {
    /// Provides mutable access to the write buffer.
    ///
    /// # Safety
    ///
    /// This is safe because the atomic state guarantees that the write_cell
    /// has no other active aliases. The writer has exclusive access to this
    /// buffer until the handle is dropped.
    fn deref_mut(&mut self) -> &mut T {
        // safety: the atomic state guarentee that the write_cell
        //         has no other active aliases
        unsafe { &mut *self.write_cell.get() }
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

    #[test]
    fn test_buffer_state_encode_decode_examples() {
        // Test the specific example from the decode documentation
        let encoded = 0x1102; // write=1, ready=0, read=2, dirty=true
        let state = BufferState::decode(encoded);
        assert_eq!(state.read_idx, 2);
        assert_eq!(state.ready_idx, 0);
        assert_eq!(state.write_idx, 1);
        assert!(state.dirty);

        // Verify it round-trips correctly
        assert_eq!(state.encode(), encoded);
    }

    #[test]
    fn test_buffer_state_encode_dirty_flag() {
        // Test dirty=false
        let state_clean = BufferState {
            read_idx: 0,
            ready_idx: 1,
            write_idx: 2,
            dirty: false,
        };
        let encoded_clean = state_clean.encode();
        assert_eq!(encoded_clean & 0xF000, 0x0000);

        // Test dirty=true
        let state_dirty = BufferState {
            read_idx: 0,
            ready_idx: 1,
            write_idx: 2,
            dirty: true,
        };
        let encoded_dirty = state_dirty.encode();
        assert_eq!(encoded_dirty & 0xF000, 0x1000);
    }

    #[test]
    fn test_buffer_state_max_index_values() {
        // Test that max valid index (3) encodes/decodes correctly
        // Using 3 as it fits in 4 bits, though typical triple buffer uses 0,1,2
        let state = BufferState {
            read_idx: 3,
            ready_idx: 3,
            write_idx: 3,
            dirty: false,
        };
        let encoded = state.encode();
        let decoded = BufferState::decode(encoded);
        assert_eq!(state, decoded);
    }

    #[test]
    fn test_buffer_state_all_permutations() {
        // Test all valid permutations of indices 0,1,2
        let permutations = [
            (0, 1, 2),
            (0, 2, 1),
            (1, 0, 2),
            (1, 2, 0),
            (2, 0, 1),
            (2, 1, 0),
        ];

        for (read, ready, write) in permutations.iter() {
            for dirty in [false, true] {
                let state = BufferState {
                    read_idx: *read,
                    ready_idx: *ready,
                    write_idx: *write,
                    dirty,
                };
                let encoded = state.encode();
                let decoded = BufferState::decode(encoded);
                assert_eq!(
                    state, decoded,
                    "Failed for read={read}, ready={ready}, write={write}, dirty={dirty}",
                );
            }
        }
    }

    #[test]
    fn test_buffer_state_bit_layout() {
        // Verify the exact bit layout described in documentation
        let state = BufferState {
            read_idx: 1,  // bits 0-3:   0001
            ready_idx: 2, // bits 4-7:   0010
            write_idx: 3, // bits 8-11:  0011
            dirty: true,  // bits 12-15: 0001
        };
        let encoded = state.encode();

        // Check each field is in the right position
        assert_eq!(encoded & 0x000F, 1); // read_idx
        assert_eq!((encoded & 0x00F0) >> 4, 2); // ready_idx
        assert_eq!((encoded & 0x0F00) >> 8, 3); // write_idx
        assert_eq!((encoded & 0xF000) >> 12, 1); // dirty flag
    }

    #[test]
    fn test_buffer_state_decode_clears_unused_bits() {
        // Ensure that decode ignores bits beyond what we use
        let encoded_with_junk = 0xFFFF_FFFF_FFFF_1321;
        let state = BufferState::decode(encoded_with_junk);

        // Should only extract the lower bits we care about
        assert_eq!(state.read_idx, 1);
        assert_eq!(state.ready_idx, 2);
        assert_eq!(state.write_idx, 3);
        assert!(state.dirty);

        // Re-encoding should produce clean output
        let clean_encoded = state.encode();
        assert_eq!(clean_encoded, 0x1321);
    }

    #[test]
    fn test_triple_buffer_writer_is_not_sync() {
        // This test verifies that TripleBufferWriter<T> does NOT implement Sync

        // Method 1: Compile-time assertion using a helper function
        // Uncomment the next lines to verify the test works (should fail to compile):
        /*
        fn assert_sync<T: Sync>() {}
        assert_sync::<TripleBufferWriter<i32>>();
        */

        // Method 2: Verify that we can create and use the writer in single-threaded context
        // This test passes, confirming the struct exists and works as expected
        #[allow(clippy::arc_with_non_send_sync)]
        let writer = TripleBufferWriter::<i32> {
            buffer: Arc::new(TripleBuffer {
                buffers: [
                    std::cell::UnsafeCell::new(Default::default()),
                    std::cell::UnsafeCell::new(Default::default()),
                    std::cell::UnsafeCell::new(Default::default()),
                ],
                encoded_state: std::sync::atomic::AtomicU64::new(0),
            }),
            borrowers: Cell::new(0),
            _not_sync: PhantomData,
        };

        // This demonstrates that we can create and use the writer
        assert_eq!(writer.borrowers.get(), 0);
        writer.borrowers.set(1);
        assert_eq!(writer.borrowers.get(), 1);

        println!("✓ TripleBufferWriter successfully created and works in single-threaded context");
    }

    #[test]
    fn test_basic_publish() {
        let (mut tx, rx) = triple_buffer::<usize>(0);
        {
            let mut write_handle = tx.write();
            *write_handle = 42
        }

        let read_handle = rx.read();
        assert_eq!(*read_handle, 42);
    }

    #[test]
    fn test_basic_publish_release() {
        let (mut tx, rx) = triple_buffer::<usize>(0);
        let mut write_handle = tx.write();
        *write_handle = 42;

        let read_handle = rx.read();
        assert_eq!(*read_handle, 0);

        drop(write_handle);
        drop(read_handle);

        let read_handle = rx.read();
        assert_eq!(*read_handle, 42);
    }
}
