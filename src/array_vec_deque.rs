// Copyright 2018 Scott J Maddox
//
// This file was derived from the Rust-lang VecDeque implementation,
// which had the following copyright header:
//
// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ptr;
use std::ops;
use {new_array, Array, CapacityError, Index, NoDrop, RangeArgument};

pub struct ArrayVecDeque<A: Array> {
    // tail and head are pointers into the buffer. Tail always points
    // to the first element that could be read, Head always points
    // to where data should be written.
    // If tail == head the buffer is empty. The length of the ringbuffer
    // is defined as the distance between the two.
    tail: A::Index,
    head: A::Index,
    buf: NoDrop<A>,
}

impl<A: Array> Drop for ArrayVecDeque<A> {
    fn drop(&mut self) {
        self.clear();

        // NoDrop inhibits array's drop
        // panic safety: NoDrop::drop will trigger on panic, so the inner
        // array will not drop even after panic.
    }
}

impl<A: Array> ArrayVecDeque<A> {
    pub fn new() -> Self {
        unsafe {
            Self {
                tail: Index::from(0),
                head: Index::from(0),
                buf: NoDrop::new(new_array()),
            }
        }
    }

    fn tail(&self) -> usize {
        self.tail.to_usize()
    }

    fn head(&self) -> usize {
        self.head.to_usize()
    }

    #[inline]
    pub fn len(&self) -> usize {
        count(self.tail(), self.head(), self.cap())
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tail() == self.head()
    }

    /// Returns `true` if and only if the buffer is at full capacity.
    #[inline]
    fn is_full(&self) -> bool {
        self.cap() - self.len() == 1
    }

    #[inline]
    fn cap(&self) -> usize {
        A::capacity()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap() - 1
    }

    #[inline]
    fn ptr(&self) -> *mut A::Item {
        self.buf.as_ptr() as *mut A::Item
    }

    /// Moves an element out of the buffer
    #[inline]
    unsafe fn buffer_read(&mut self, off: usize) -> A::Item {
        ptr::read(self.ptr().offset(off as isize))
    }

    /// Writes an element into the buffer, moving it.
    #[inline]
    unsafe fn buffer_write(&mut self, off: usize, value: A::Item) {
        ptr::write(self.ptr().offset(off as isize), value);
    }

    /// Returns the index in the underlying buffer for a given logical element
    /// index.
    #[inline]
    fn wrap_index(&self, idx: usize) -> usize {
        Index::from(wrap_index(idx.to_usize(), self.cap()))
    }

    /// Returns the index in the underlying buffer for a given logical element
    /// index + addend.
    #[inline]
    fn wrap_add(&self, idx: A::Index, addend: usize) -> A::Index {
        let idx = idx.to_usize();
        debug_assert!(idx < self.cap());
        debug_assert!(addend < self.cap());
        if let Some(i) = idx.checked_add(addend) {
            Index::from(self.wrap_index(i))
        } else {
            panic!("very large addends are not currently supported");
        }
    }

    /// Returns the index in the underlying buffer for a given logical element
    /// index - subtrahend.
    #[inline]
    fn wrap_sub(&self, idx: A::Index, subtrahend: usize) -> A::Index {
        let idx = idx.to_usize();
        debug_assert!(idx < self.cap());
        debug_assert!(subtrahend < self.cap());
        if let Some(i) = idx.checked_sub(subtrahend) {
            Index::from(self.wrap_index(i))
        } else {
            Index::from(self.cap() - subtrahend + idx)
        }
    }

    pub fn pop_front(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            None
        } else {
            let tail = self.tail.to_usize();
            self.tail = self.wrap_add(self.tail, 1);
            unsafe { Some(self.buffer_read(tail)) }
        }
    }

    pub fn pop_back(&mut self) -> Option<A::Item> {
        if self.is_empty() {
            None
        } else {
            self.head = self.wrap_sub(self.head, 1);
            let head = self.head.to_usize();
            unsafe { Some(self.buffer_read(head)) }
        }
    }

    pub fn try_push_back(&mut self, value: A::Item) -> Result<(), CapacityError<A::Item>> {
        if self.is_full() {
            Err(CapacityError::new(value))
        } else {
            let head = self.head.to_usize();
            self.head = self.wrap_add(self.head, 1);
            unsafe { self.buffer_write(head, value) }
            Ok(())
        }
    }

    #[inline]
    pub fn push_back(&mut self, value: A::Item) {
        self.try_push_back(value).unwrap();
    }

    pub fn try_push_front(&mut self, value: A::Item) -> Result<(), CapacityError<A::Item>> {
        if self.is_full() {
            Err(CapacityError::new(value))
        } else {
            self.tail = self.wrap_sub(self.tail, 1);
            let tail = self.tail.to_usize();
            unsafe {
                self.buffer_write(tail, value);
            }
            Ok(())
        }
    }

    #[inline]
    pub fn push_front(&mut self, value: A::Item) {
        self.try_push_front(value).unwrap();
    }

    /// Clears the buffer, removing all values.
    #[inline]
    pub fn clear(&mut self) {
        while let Some(_) = self.pop_front() {}
    }

    pub fn get(&self, index: usize) -> Option<&A::Item> {
        if index < self.len() {
            let idx = self.wrap_add(self.tail, index).to_usize();
            unsafe { Some(&*self.ptr().offset(idx as isize)) }
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut A::Item> {
        if index < self.len() {
            let idx = self.wrap_add(self.tail, index).to_usize();
            unsafe { Some(&mut *self.ptr().offset(idx as isize)) }
        } else {
            None
        }
    }
}

impl<A: Array> ops::Index<usize> for ArrayVecDeque<A> {
    type Output = A::Item;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("Out of bounds access")
    }
}

impl<A: Array> ops::IndexMut<usize> for ArrayVecDeque<A> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("Out of bounds access")
    }
}

/// Calculate the number of elements left to be read in the buffer
#[inline]
fn count(tail: usize, head: usize, size: usize) -> usize {
    if tail <= head {
        head - tail
    } else {
        size - tail + head
    }
}

/// Returns the index in the underlying buffer for a given logical element index.
#[inline]
fn wrap_index(index: usize, size: usize) -> usize {
    index % size
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn new_push_pop_index() {
        let mut v = ArrayVecDeque::<[usize; 10]>::new();
        v.push_back(1);
        assert_eq!(v.pop_back().unwrap(), 1);
        v.push_front(2);
        assert_eq!(v.pop_front().unwrap(), 2);
        v.push_back(1);
        v.push_back(2);
        assert_eq!(v[0], 1);
        assert_eq!(v[1], 2);
    }

    #[test]
    fn wrap_around() {
        let mut v = ArrayVecDeque::<[usize; 3]>::new();
        assert_eq!(v.len(), 0);
        v.push_back(1);
        assert_eq!(v.len(), 1);
        v.push_back(2);
        assert_eq!(v.len(), 2);
        assert_eq!(v.pop_front().unwrap(), 1);
        assert_eq!(v.len(), 1);
        v.push_back(3);
        assert_eq!(v.len(), 2);
        assert_eq!(v.pop_front().unwrap(), 2);
        assert_eq!(v.len(), 1);
        assert_eq!(v.pop_front().unwrap(), 3);
        assert_eq!(v.len(), 0);
    }
}
