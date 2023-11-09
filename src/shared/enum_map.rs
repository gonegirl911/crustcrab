pub use macros::{Display, Enum};

use generic_array::{
    functional::FunctionalSequence,
    sequence::GenericSequence,
    typenum::{bit::B1, Add1, Unsigned},
    ArrayLength, GenericArray, GenericArrayIter,
};
use serde::{
    de::{Error, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{
    fmt,
    iter::Enumerate,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ops::{Add, Index, IndexMut},
    slice,
};

#[macro_export]
macro_rules! enum_map {
    ($($t:tt)*) => {
        $crate::shared::enum_map::EnumMap::from_fn(|variant| match variant {
            $($t)*
        })
    };
}

pub struct EnumMap<E: Enum, T>(GenericArray<T, E::Length>);

impl<E: Enum, T> EnumMap<E, T> {
    pub fn from_fn<F: FnMut(E) -> T>(mut f: F) -> Self {
        Self(GenericArray::generate(|i| {
            f(unsafe { E::from_index_unchecked(i) })
        }))
    }

    fn uninit() -> EnumMap<E, MaybeUninit<T>> {
        EnumMap(GenericArray::uninit())
    }

    pub fn values(&self) -> slice::Iter<T> {
        self.0.iter()
    }

    pub fn into_values(self) -> GenericArrayIter<T, E::Length> {
        self.0.into_iter()
    }

    pub fn map<U, F>(self, mut f: F) -> EnumMap<E, U>
    where
        F: FnMut(E, T) -> U,
    {
        let mut variants = E::variants();
        EnumMap(
            self.0
                .map(|value| f(unsafe { variants.next_unchecked() }, value)),
        )
    }
}

impl<E: Enum, T> EnumMap<E, MaybeUninit<T>> {
    unsafe fn assume_init(self) -> EnumMap<E, T> {
        EnumMap(unsafe { GenericArray::assume_init(self.0) })
    }
}

impl<E: Enum, T> FromIterator<(E, T)> for EnumMap<E, T> {
    fn from_iter<I: IntoIterator<Item = (E, T)>>(iter: I) -> Self {
        let mut uninit = EnumMap::uninit();
        let mut guard = Guard::new(&mut uninit);

        for (variant, value) in iter {
            guard.write(variant, value);
        }

        if guard.finish() {
            unsafe { uninit.assume_init() }
        } else {
            panic!("missing variants");
        }
    }
}

impl<E: Enum, T: Clone> Clone for EnumMap<E, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: Enum, T: Clone> Copy for EnumMap<E, T> where GenericArray<T, E::Length>: Copy {}

impl<E: Enum, T: Default> Default for EnumMap<E, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<E: Enum, T> Index<E> for EnumMap<E, T> {
    type Output = T;

    fn index(&self, variant: E) -> &Self::Output {
        unsafe { self.0.get_unchecked(variant.to_index()) }
    }
}

impl<E: Enum, T> IndexMut<E> for EnumMap<E, T> {
    fn index_mut(&mut self, variant: E) -> &mut Self::Output {
        unsafe { self.0.get_unchecked_mut(variant.to_index()) }
    }
}

impl<E: Enum, T> IntoIterator for EnumMap<E, T> {
    type Item = (E, T);
    type IntoIter = IntoIter<E, T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter(self.0.into_iter().enumerate())
    }
}

pub struct IntoIter<E: Enum, T>(Enumerate<GenericArrayIter<T, E::Length>>);

impl<E: Enum, T> Iterator for IntoIter<E, T> {
    type Item = (E, T);

    fn next(&mut self) -> Option<Self::Item> {
        self.0
            .next()
            .map(|(i, value)| (unsafe { E::from_index_unchecked(i) }, value))
    }
}

impl<'a, E: Enum, T> IntoIterator for &'a EnumMap<E, T> {
    type Item = (E, &'a T);
    type IntoIter = Iter<'a, E, T>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            inner: self.0.iter().enumerate(),
            phantom: PhantomData,
        }
    }
}

pub struct Iter<'a, E: Enum, T> {
    inner: Enumerate<slice::Iter<'a, T>>,
    phantom: PhantomData<fn() -> (E, &'a T)>,
}

impl<'a, E: Enum, T> Iterator for Iter<'a, E, T> {
    type Item = (E, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(i, value)| (unsafe { E::from_index_unchecked(i) }, value))
    }
}

impl<'de, E, T> Deserialize<'de> for EnumMap<E, T>
where
    E: Enum + Deserialize<'de>,
    T: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<E: Enum, T>(PhantomData<fn() -> EnumMap<E, T>>);

        impl<'de, E, T> Visitor<'de> for MapVisitor<E, T>
        where
            E: Enum + Deserialize<'de>,
            T: Deserialize<'de>,
        {
            type Value = EnumMap<E, T>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "a map")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
                let mut uninit = EnumMap::uninit();
                let mut guard = Guard::new(&mut uninit);

                while let Some((variant, value)) = access.next_entry()? {
                    if guard.init(variant, value).is_err() {
                        return Err(M::Error::custom("duplicate variant"));
                    }
                }

                if guard.finish() {
                    Ok(unsafe { uninit.assume_init() })
                } else {
                    Err(M::Error::custom("missing variants"))
                }
            }
        }

        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

struct Guard<'a, E: Enum, T> {
    uninit: &'a mut EnumMap<E, MaybeUninit<T>>,
    is_init: EnumMap<E, bool>,
    count: usize,
}

impl<'a, E: Enum, T> Guard<'a, E, T> {
    fn new(uninit: &'a mut EnumMap<E, MaybeUninit<T>>) -> Self {
        Self {
            uninit,
            is_init: Default::default(),
            count: 0,
        }
    }

    fn init(&mut self, variant: E, value: T) -> Result<(), T> {
        if !mem::replace(&mut self.is_init[variant], true) {
            self.uninit[variant].write(value);
            self.count += 1;
            Ok(())
        } else {
            Err(value)
        }
    }

    fn write(&mut self, variant: E, value: T) {
        if let Err(value) = self.init(variant, value) {
            *unsafe { self.uninit[variant].assume_init_mut() } = value;
        }
    }

    fn finish(self) -> bool {
        if self.count == E::LEN {
            mem::forget(self);
            true
        } else {
            false
        }
    }
}

impl<E: Enum, T> Drop for Guard<'_, E, T> {
    fn drop(&mut self) {
        for (variant, &is_init) in &self.is_init {
            if is_init {
                unsafe {
                    self.uninit[variant].assume_init_drop();
                }
            }
        }
    }
}

/// # Safety
///
/// `to_index` must return an index in the range `[0, Self::LEN)`.
pub unsafe trait Enum: Copy {
    type Length: ArrayLength;

    const LEN: usize = Self::Length::USIZE;

    fn from_index(index: usize) -> Option<Self> {
        (index < Self::LEN).then(|| unsafe { Self::from_index_unchecked(index) })
    }

    unsafe fn from_index_unchecked(index: usize) -> Self;

    fn to_index(self) -> usize;

    fn variants() -> Variants<Self> {
        Variants {
            index: 0,
            phantom: PhantomData,
        }
    }
}

unsafe impl<E: Enum> Enum for Option<E>
where
    E::Length: Add<B1>,
    Add1<E::Length>: ArrayLength,
{
    type Length = Add1<E::Length>;

    unsafe fn from_index_unchecked(index: usize) -> Self {
        E::from_index(index)
    }

    fn to_index(self) -> usize {
        self.map_or(E::LEN, Enum::to_index)
    }
}

pub struct Variants<E> {
    index: usize,
    phantom: PhantomData<fn() -> E>,
}

impl<E: Enum> Variants<E> {
    unsafe fn next_unchecked(&mut self) -> <Self as Iterator>::Item {
        let value = unsafe { E::from_index_unchecked(self.index) };
        self.index += 1;
        value
    }
}

impl<E: Enum> Iterator for Variants<E> {
    type Item = E;

    fn next(&mut self) -> Option<Self::Item> {
        (self.index < E::LEN).then(|| unsafe { self.next_unchecked() })
    }
}
