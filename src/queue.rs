use std::cell::UnsafeCell;
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::{self, AtomicUsize, Ordering};
use core::cell::Cell;

// A slot in a queue
struct Slot<T> {
    /// The stamp tracks the state of this slot
    stamp: AtomicUsize,
    /// The value stored in this slot
    value: UnsafeCell<MaybeUninit<T>>,
}

/// A bounded multi-producer multi-consumer lock-free queue
pub struct ArrayQueue<T> {
    /// The head of the queue (where elements are popped from)
    head: CachePadded<AtomicUsize>,
    
    /// The tail of the queue (where elements are pushed to)
    tail: CachePadded<AtomicUsize>,
    
    /// Buffer holding the slots
    buffer: Box<[Slot<T>]>,
    
    /// A stamp with the value representing one complete lap
    one_lap: usize,
}

unsafe impl<T: Send> Sync for ArrayQueue<T> {}
unsafe impl<T: Send> Send for ArrayQueue<T> {}

impl<T> ArrayQueue<T> {
    /// Creates a new bounded queue with the given capacity
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be non-zero");

        // Head and tail both start at index 0
        let head = 0;
        let tail = 0;

        // Allocate a buffer of slots initialized with stamps
        let buffer: Box<[Slot<T>]> = (0..cap)
            .map(|i| {
                Slot {
                    stamp: AtomicUsize::new(i),
                    value: UnsafeCell::new(MaybeUninit::uninit()),
                }
            })
            .collect();

        // One lap is the smallest power of two greater than cap
        let one_lap = (cap + 1).next_power_of_two();

        Self {
            buffer,
            one_lap,
            head: CachePadded::new(AtomicUsize::new(head)),
            tail: CachePadded::new(AtomicUsize::new(tail)),
        }
    }

    /// Attempts to push an element into the queue
    pub fn push(&self, value: T) -> Result<(), T> {
        let backoff = Backoff::new();
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            // Decode the tail position
            let index = tail & (self.one_lap - 1);
            let lap = tail & !(self.one_lap - 1);

            // Determine the next tail position
            let next_tail = if index + 1 < self.capacity() {
                // Same lap, incremented index
                tail + 1
            } else {
                // New lap, index wraps to zero
                lap.wrapping_add(self.one_lap)
            };

            // Get the slot at the current index
            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            // If the tail and stamp match, we can try to push
            if tail == stamp {
                // Try to claim this slot by updating the tail
                match self.tail.compare_exchange_weak(
                    tail,
                    next_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // We've claimed the slot, now write the value
                        unsafe {
                            ptr::write((*slot.value.get()).as_mut_ptr(), value);
                        }
                        // Update the stamp to indicate this slot is ready for reading
                        slot.stamp.store(tail + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(t) => {
                        // Another thread moved the tail, try again
                        tail = t;
                        backoff.spin();
                    }
                }
            } else if stamp.wrapping_add(self.one_lap) == tail + 1 {
                // This slot is one lap ahead, which could indicate queue fullness
                atomic::fence(Ordering::SeqCst);
                let head = self.head.load(Ordering::Relaxed);

                // If head is a full lap behind, the queue is full
                if head.wrapping_add(self.one_lap) == tail {
                    return Err(value);
                }

                backoff.spin();
                tail = self.tail.load(Ordering::Relaxed);
            } else {
                // We need to wait for the stamp to get updated
                backoff.snooze();
                tail = self.tail.load(Ordering::Relaxed);
            }
        }
    }

    /// Attempts to pop an element from the queue
    pub fn pop(&self) -> Option<T> {
        let backoff = Backoff::new();
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            // Decode the head position
            let index = head & (self.one_lap - 1);
            let lap = head & !(self.one_lap - 1);

            // Get the slot at the current index
            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            // If the stamp is ahead of the head by 1, we can try to pop
            if head + 1 == stamp {
                // Determine the next head position
                let next = if index + 1 < self.capacity() {
                    // Same lap, incremented index
                    head + 1
                } else {
                    // New lap, index wraps to zero
                    lap.wrapping_add(self.one_lap)
                };

                // Try to claim this slot by updating the head
                match self.head.compare_exchange_weak(
                    head,
                    next,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // We've claimed the slot, now read the value
                        let value = unsafe {
                            let value = ptr::read((*slot.value.get()).as_ptr());
                            // Update the stamp to indicate this slot can be reused
                            slot.stamp.store(head.wrapping_add(self.one_lap), Ordering::Release);
                            value
                        };
                        return Some(value);
                    }
                    Err(h) => {
                        // Another thread moved the head, try again
                        head = h;
                        backoff.spin();
                    }
                }
            } else if stamp == head {
                // The head and stamp match, check if queue is empty
                atomic::fence(Ordering::SeqCst);
                let tail = self.tail.load(Ordering::Relaxed);

                // If tail equals head, the queue is empty
                if tail == head {
                    return None;
                }

                backoff.spin();
                head = self.head.load(Ordering::Relaxed);
            } else {
                // We need to wait for the stamp to get updated
                backoff.snooze();
                head = self.head.load(Ordering::Relaxed);
            }
        }
    }

    /// Returns the capacity of the queue
    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// Returns true if the queue is empty
    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);
        tail == head
    }

    /// Returns true if the queue is full
    pub fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::SeqCst);
        let head = self.head.load(Ordering::SeqCst);
        head.wrapping_add(self.one_lap) == tail
    }
}

impl<T> Drop for ArrayQueue<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            // Drop all values between head and tail
            let head = *self.head.get_mut();
            let tail = *self.tail.get_mut();

            let hix = head & (self.one_lap - 1);
            let tix = tail & (self.one_lap - 1);

            let len = if hix < tix {
                tix - hix
            } else if hix > tix {
                self.capacity() - hix + tix
            } else if tail == head {
                0
            } else {
                self.capacity()
            };

            // Drop all items in the queue
            for i in 0..len {
                let index = if hix + i < self.capacity() {
                    hix + i
                } else {
                    hix + i - self.capacity()
                };

                unsafe {
                    let slot = self.buffer.get_unchecked_mut(index);
                    ptr::drop_in_place((*slot.value.get()).as_mut_ptr());
                }
            }
        }
    }
}

impl<T> fmt::Debug for ArrayQueue<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("ArrayQueue { .. }")
    }
}

// Backoff implementation for efficient spinning
const SPIN_LIMIT: u32 = 6;
const YIELD_LIMIT: u32 = 10;

pub struct Backoff {
    step: Cell<u32>,
}

impl Backoff {
    #[inline]
    pub fn new() -> Self {
        Self { step: Cell::new(0) }
    }

    #[inline]
    pub fn reset(&self) {
        self.step.set(0);
    }

    #[inline]
    pub fn spin(&self) {
        for _ in 0..1 << self.step.get().min(SPIN_LIMIT) {
            std::hint::spin_loop();
        }

        if self.step.get() <= SPIN_LIMIT {
            self.step.set(self.step.get() + 1);
        }
    }

    #[inline]
    pub fn snooze(&self) {
        if self.step.get() <= SPIN_LIMIT {
            for _ in 0..1 << self.step.get() {
                std::hint::spin_loop();
            }
        } else {
            std::thread::yield_now();
        }

        if self.step.get() <= YIELD_LIMIT {
            self.step.set(self.step.get() + 1);
        }
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        self.step.get() > YIELD_LIMIT
    }
}

// Cache-padded value to prevent false sharing
#[repr(align(128))]
pub struct CachePadded<T> {
    value: T,
}

impl<T> CachePadded<T> {
    #[inline]
    pub const fn new(t: T) -> Self {
        Self { value: t }
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.value
    }
}

impl<T> std::ops::Deref for CachePadded<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        &self.value
    }
}

impl<T> std::ops::DerefMut for CachePadded<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        &mut self.value
    }
}
