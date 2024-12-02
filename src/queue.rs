use crate::errors::OutOfBoundsError;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum RepeatMode {
    Off,
    Single,
    All,
}

impl RepeatMode {
    pub fn next(&self) -> Self {
        match *self {
            Self::All => Self::Single,
            Self::Single => Self::Off,
            Self::Off => Self::All,
        }
    }
}

/// A queue of items with variable iteration rules, depending on the repeat_mode field.
///
/// Queues like this are commonly found in music players, such as spotify or youtube music.
///
/// See [`next`] for an explanation on how the repeat mode changes iteration.
///
/// [`next`]: Self::next
#[derive(Clone, Debug)]
pub struct Queue<T> {
    items: Vec<T>,
    index: usize,
    /// The repeat mode of the queue.
    ///
    /// The value is used in [`next`] to determine which item to return.
    ///
    /// [`next`]: Self::next
    pub repeat_mode: RepeatMode,
    /// Used for proper iteration after skipping/jumping, and initial `next` call
    has_advanced: bool,
}

impl<T> Queue<T> {
    /// Create a new queue with the given repeat mode.
    pub fn new(repeat_mode: RepeatMode) -> Self {
        Self {
            items: Vec::new(),
            index: 0,
            repeat_mode,
            has_advanced: false,
        }
    }

    /// Return the next item in the queue, depending on the [RepeatMode].
    ///
    /// If the queue is not empty, the first call is gives the first item, regardless of the repeat mode.
    /// Calling [`next`] after [`jump`], [`skip`] or [`rewind`] also guarantees the next item, regardless of the repeat mode.
    ///
    /// [`next`]: Self::next
    /// [`jump`]: Self::jump
    /// [`skip`]: Self::skip
    /// [`rewind`]: Self::rewind
    ///
    /// # Iteration rules
    /// 
    /// If we reach the end of the queue, and the repeat mode is [`All`], the queue will wrap around to the beginning.
    ///
    /// If it is [`Single`], the same element will be returned every time.
    ///
    /// If it is [`Off`], this method will return None, and the internal index will not be updated.
    /// 
    /// [`All`]: RepeatMode::All
    /// [`Single`]: RepeatMode::Single
    /// [`Off`]: RepeatMode::Off
    pub fn next(&mut self) -> Option<&T> {
        if self.items.is_empty() {
            return None;
        }
        if self.repeat_mode != RepeatMode::Single && self.has_advanced && self.index < self.items.len() {
            self.index += 1;
        }
        if self.repeat_mode != RepeatMode::Off {
            self.index %= self.items.len();
        }
        self.has_advanced = true;
        self.items.get(self.index)
    }

    /// Push a value onto the internal vector
    pub fn push(&mut self, value: T) {
        self.items.push(value)
    }

    /// Extend the internal vector by the iterator
    pub fn extend<I>(&mut self, iter: I)
    where
        I: IntoIterator<Item = T>,
    {
        self.items.extend(iter);
    }

    /// Remove the value at `index`, calling [`Vec::remove`] internally.
    ///
    /// Rewinds the queue by 1 if the given index is less than the internal one.
    pub fn remove(&mut self, index: usize) {
        self.items.remove(index);
        if index < self.index {
            self.index -= 1;
        }
    }

    /// Jump to index `n` in the queue.
    ///
    /// This method guarantees the next item is at index `n`.
    pub fn jump(&mut self, new_index: usize) -> Result<(), OutOfBoundsError<usize>> {
        if new_index > self.items.len() {
            return Err(OutOfBoundsError::High {
                value: new_index,
                max: self.items.len(),
            });
        }
        self.has_advanced = false;
        self.index = new_index;
        Ok(())
    }

    /// Skip n items forward.
    ///
    /// This method guarantees the next item is `n` ahead.
    ///
    /// If the repeat mode is Off, skipping beyond the end of the queue will set the index to the length of the queue, otherwise wrap around to the beginning.
    pub fn skip(&mut self, n: usize) {
        let new_index = if !self.items.is_empty()
            || (self.items.len() - self.index) > n && self.repeat_mode == RepeatMode::Off
        {
            self.items.len()
        } else {
            (self.index + n) % self.items.len()
        };
        self.jump(new_index)
            .expect("Calculated jump from skip shouldn't fail");
    }

    /// Rewind n items.
    ///
    /// This method guarantees the next item is `n` behind the current item.
    pub fn rewind(&mut self, n: usize) {
        let new_index = if self.items.is_empty() {
            0
        } else if n <= self.index {
            self.index - n
        } else {
            self.items.len() - (n - self.index)
        };
        self.jump(new_index)
            .expect("Calculated jump from rewind shouldn't fail");
    }

    /// Return a reference to the element that was last returned.
    pub fn current(&self) -> Option<&T> {
        self.items.get(self.index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_iteration_all_test() {
        let mut queue: Queue<u32> = Queue::new(RepeatMode::All);
        queue.items = vec![1, 2, 3];
        assert_eq!(queue.next(), Some(&1));
        assert_eq!(queue.next(), Some(&2));
        assert_eq!(queue.next(), Some(&3));
        assert_eq!(queue.next(), Some(&1));
        assert_eq!(queue.next(), Some(&2));
    }

    #[test]
    fn queue_iteration_off_test() {
        let mut queue: Queue<u32> = Queue::new(RepeatMode::Off);
        queue.items = vec![1, 2, 3];
        assert_eq!(queue.next(), Some(&1));
        assert_eq!(queue.next(), Some(&2));
        assert_eq!(queue.next(), Some(&3));
        assert_eq!(queue.next(), None);
        assert_eq!(queue.next(), None);
    }

    #[test]
    fn queue_iteration_single_test() {
        let mut queue: Queue<u32> = Queue::new(RepeatMode::Single);
        queue.items = vec![1, 2, 3];
        assert_eq!(queue.next(), Some(&1));
        assert_eq!(queue.next(), Some(&1));
        assert_eq!(queue.next(), Some(&1));
    }
}
