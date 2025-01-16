use core::fmt;
use core::ops::{Deref, DerefMut};

#[cfg(not(feature = "alloc"))]
type Inner<T, const N: usize> = heapless::Vec<T, N>;
#[cfg(feature = "alloc")]
type Inner<T, const N: usize> = alloc::vec::Vec<T>;

#[derive(Default, Clone)]
pub struct Vec<T, const N: usize> {
    inner: Inner<T, N>,
}

impl<T, const N: usize> Vec<T, N> {
    pub fn new() -> Self {
        Self {
            inner: Inner::new(),
        }
    }

    #[cfg(not(feature = "alloc"))]
    pub fn push(&mut self, value: T) -> Result<(), ()> {
        self.inner.push(value).map_err(|_| ())
    }

    #[cfg(not(feature = "alloc"))]
    pub fn insert(&mut self, index: usize, element: T) -> Result<(), ()> {
        self.inner.insert(index, element).map_err(|_| ())
    }

    #[cfg(feature = "alloc")]
    pub fn push(&mut self, element: T) -> Result<(), ()> {
        self.inner.push(element);
        Ok(())
    }

    #[cfg(feature = "alloc")]
    pub fn insert(&mut self, index: usize, element: T) -> Result<(), ()> {
        self.inner.insert(index, element);
        Ok(())
    }

    pub fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.inner.extend(iter)
    }
}

impl<T: fmt::Debug, const N: usize> fmt::Debug for Vec<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Vec").field(&self.inner).finish()
    }
}

impl<T, const N: usize> Deref for Vec<T, N> {
    type Target = Inner<T, N>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<T, const N: usize> DerefMut for Vec<T, N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<T: PartialEq, const N: usize> PartialEq for Vec<T, N> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: Eq, const N: usize> Eq for Vec<T, N> {}

impl<T: defmt::Format, const N: usize> defmt::Format for Vec<T, N> {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "[");
        let mut first = true;
        for item in &self.inner {
            if !first {
                defmt::write!(fmt, ", ");
            }
            first = false;
            defmt::write!(fmt, "{}", item);
        }
        defmt::write!(fmt, "]");
    }
}
