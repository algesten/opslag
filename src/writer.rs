use core::ops::{Deref, DerefMut};

use crate::vec::Vec;

#[derive(Debug)]
struct LabelOffset<'b>(&'b str, usize);

pub struct Writer<'a, 'b, const LK: usize = 10> {
    output: &'b mut [u8],
    position: usize,
    overflow: bool,
    lookup: Vec<LabelOffset<'a>, LK>,
}

impl<'a, 'b, const LK: usize> Writer<'a, 'b, LK> {
    pub fn new(buffer: &'b mut [u8]) -> Self {
        Self {
            output: buffer,
            position: 0,
            overflow: false,
            lookup: Vec::new(),
        }
    }

    pub fn into_inner(self) -> &'b mut [u8] {
        &mut self.output[..self.position]
    }

    pub fn len(&self) -> usize {
        self.position
    }

    pub fn is_overflow(&self) -> bool {
        self.overflow
    }

    pub(crate) fn write(&mut self, data: &[u8]) {
        if self.overflow {
            return;
        }
        let len = data.len();
        if self.position + len > self.output.len() {
            self.overflow = true;
            return;
        }
        self.output[self.position..self.position + len].copy_from_slice(data);
        self.position += len;
    }

    pub(crate) fn write_u8(&mut self, b: u8) {
        if self.overflow {
            return;
        }
        if self.position >= self.output.len() {
            self.overflow = true;
            return;
        }
        self.output[self.position] = b;
        self.position += 1;
    }

    pub(crate) fn inc(&mut self, v: usize) {
        if self.overflow {
            return;
        }
        if self.position + v > self.output.len() {
            self.overflow = true;
            return;
        }
        self.position += v;
    }

    pub(crate) fn find_label(&mut self, label: &str) -> Option<usize> {
        self.lookup.iter().find(|o| o.0 == label).map(|o| o.1)
    }

    pub(crate) fn push_label(&mut self, label: &'a str, offset: usize) {
        let off = LabelOffset(label, self.position + offset);
        // If it overflow, we simply can't store more offsets, which is fine.
        let _ = self.lookup.push(off);
    }

    pub(crate) fn reserve(&mut self, len: usize) -> Reservation {
        let r = Reservation {
            start: self.position,
            len,
        };
        self.inc(len);
        r
    }

    pub(crate) fn distance_from_reservation(&self, r: &Reservation) -> usize {
        self.position - r.start
    }

    pub(crate) fn write_reservation(&mut self, r: Reservation, data: &[u8]) {
        if self.overflow {
            return;
        }
        self.output[r.start..(r.start + r.len)].copy_from_slice(data);
    }
}

pub(crate) struct Reservation {
    start: usize,
    len: usize,
}

impl<const LK: usize> Deref for Writer<'_, '_, LK> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        if self.overflow {
            return &[];
        }
        &self.output[self.position..]
    }
}

impl<const LK: usize> DerefMut for Writer<'_, '_, LK> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        if self.overflow {
            return &mut [];
        }
        &mut self.output[self.position..]
    }
}
