use std::cell::UnsafeCell;
use std::fmt;
use std::mem::{self, MaybeUninit};
use std::ptr;
use std::sync::atomic::{self, AtomicUsize, Ordering};
use core::cell::Cell;

struct Slot<T> {
    stamp: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

/// A bounded multi-producer multi-consumer lock-free queue
pub struct ArrayQueue<T> {
    head: CachePadded<AtomicUsize>,
    tail: CachePadded<AtomicUsize>,
    buffer: Box<[Slot<T>]>,
    generation: usize,
}

unsafe impl<T: Send> Sync for ArrayQueue<T> {}
unsafe impl<T: Send> Send for ArrayQueue<T> {}

impl<T> ArrayQueue<T> {
    /// Creates a new bounded queue with the given capacity
    pub fn new(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be non-zero");
        let head = 0;
        let tail = 0;
        let buffer: Box<[Slot<T>]> = (0..cap)
            .map(|i| {
                Slot {
                    stamp: AtomicUsize::new(i),
                    value: UnsafeCell::new(MaybeUninit::uninit()),
                }
            })
            .collect();
        let generation = (cap + 1).next_power_of_two();

        Self {
            buffer,
            generation,
            head: CachePadded::new(AtomicUsize::new(head)),
            tail: CachePadded::new(AtomicUsize::new(tail)),
        }
    }

    pub fn push(&self, value: T) -> Result<(), T> {
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            let index = tail & (self.generation - 1);
            let lap = tail & !(self.generation - 1);

            let next_tail = if index + 1 < self.capacity() {
                tail + 1
            } else {
                lap.wrapping_add(self.generation)
            };

            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            if tail == stamp {
                match self.tail.compare_exchange_weak(
                    tail,
                    next_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        unsafe {
                            ptr::write((*slot.value.get()).as_mut_ptr(), value);
                        }
                        slot.stamp.store(tail + 1, Ordering::Release);
                        return Ok(());
                    }
                    Err(t) => {
                        tail = t;
                    }
                }
            } else if stamp.wrapping_add(self.generation) == tail + 1 {
                atomic::fence(Ordering::SeqCst);
                let head = self.head.load(Ordering::Relaxed);

                if head.wrapping_add(self.generation) == tail {
                    return Err(value);
                }

                tail = self.tail.load(Ordering::Relaxed);
            } else {
                tail = self.tail.load(Ordering::Relaxed);
            }
        }
    }

    /// Attempts to pop an element from the queue
    pub fn pop(&self) -> Option<T> {
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            let index = head & (self.generation - 1);
            let lap = head & !(self.generation - 1);

            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            if head + 1 == stamp {
                let next = if index + 1 < self.capacity() {
                    head + 1
                } else {
                    lap.wrapping_add(self.generation)
                };

                match self.head.compare_exchange_weak(
                    head,
                    next,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        let value = unsafe {
                            let value = ptr::read((*slot.value.get()).as_ptr());
                            slot.stamp.store(head.wrapping_add(self.generation), Ordering::Release);
                            value
                        };
                        return Some(value);
                    }
                    Err(h) => {
                        head = h;
                    }
                }
            } else if stamp == head {
                atomic::fence(Ordering::SeqCst);
                let tail = self.tail.load(Ordering::Relaxed);

                if tail == head {
                    return None;
                }

                head = self.head.load(Ordering::Relaxed);
            } else {
                head = self.head.load(Ordering::Relaxed);
            }
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    pub fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);
        tail == head
    }

    pub fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::SeqCst);
        let head = self.head.load(Ordering::SeqCst);
        head.wrapping_add(self.generation) == tail
    }
}

impl<T> Drop for ArrayQueue<T> {
    fn drop(&mut self) {
        if mem::needs_drop::<T>() {
            let head = *self.head.get_mut();
            let tail = *self.tail.get_mut();

            let hix = head & (self.generation - 1);
            let tix = tail & (self.generation - 1);

            let len = if hix < tix {
                tix - hix
            } else if hix > tix {
                self.capacity() - hix + tix
            } else if tail == head {
                0
            } else {
                self.capacity()
            };

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
