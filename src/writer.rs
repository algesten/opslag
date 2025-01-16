use core::ops::{Deref, DerefMut};

use crate::vec::Vec;

#[derive(Debug)]
struct LabelOffset<'b>(&'b str, usize);

pub struct Writer<'a, 'b, const LK: usize = 10> {
    output: &'b mut [u8],
    position: usize,
    lookup: Vec<LabelOffset<'a>, LK>,
}

impl<'a, 'b, const LK: usize> Writer<'a, 'b, LK> {
    pub fn new(buffer: &'b mut [u8]) -> Self {
        Self {
            output: buffer,
            position: 0,
            lookup: Vec::new(),
        }
    }

    pub fn into_inner(self) -> &'b mut [u8] {
        &mut self.output[..self.position]
    }

    pub(crate) fn inc(&mut self, v: usize) {
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
        self.output[r.start..(r.start + r.len)].copy_from_slice(data);
    }
}

pub(crate) struct Reservation {
    start: usize,
    len: usize,
}

impl<'a, 'b, const LK: usize> Deref for Writer<'a, 'b, LK> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.output[self.position..]
    }
}

impl<'a, 'b, const LK: usize> DerefMut for Writer<'a, 'b, LK> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.output[self.position..]
    }
}
