#![feature(impl_trait_in_assoc_type, mut_restriction, type_alias_impl_trait)]

mod builder;
mod enum_trait;
mod serde;

pub use builder::Builder;
pub use enum_trait::Enum;
pub use generic_array;

use generic_array::{
    GenericArray, GenericArrayIter, functional::FunctionalSequence, sequence::GenericSequence,
};
use std::{
    mem::MaybeUninit,
    ops::{Index, IndexMut},
    slice,
};

pub struct EnumMap<E: Enum, T>(GenericArray<T, E::Length>);

impl<E: Enum, T> EnumMap<E, T> {
    pub fn from_fn<F: FnMut(E) -> T>(mut f: F) -> Self {
        Self(GenericArray::generate(|i| {
            f(unsafe { E::from_index_unchecked(i) })
        }))
    }

    fn builder() -> Builder<E, T> {
        Default::default()
    }

    fn uninit() -> EnumMap<E, MaybeUninit<T>> {
        EnumMap(GenericArray::uninit())
    }

    fn iter(&self) -> impl Iterator<Item = (E, &T)> {
        E::variants().zip(&self.0)
    }

    fn values(&self) -> slice::Iter<'_, T> {
        self.0.iter()
    }

    pub fn values_mut(&mut self) -> slice::IterMut<'_, T> {
        self.0.iter_mut()
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
                .map(|value| f(unsafe { variants.next().unwrap_unchecked() }, value)),
        )
    }

    pub fn inner(&self) -> &GenericArray<T, E::Length> {
        &self.0
    }
}

impl<E: Enum, T> EnumMap<E, MaybeUninit<T>> {
    unsafe fn assume_init(self) -> EnumMap<E, T> {
        EnumMap(unsafe { GenericArray::assume_init(self.0) })
    }
}

impl<E: Enum, T: Default> Default for EnumMap<E, T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<E: Enum, T> FromIterator<(E, T)> for EnumMap<E, T> {
    fn from_iter<I: IntoIterator<Item = (E, T)>>(iter: I) -> Self {
        let mut builder = EnumMap::builder();
        for (variant, value) in iter {
            builder.set(variant, value);
        }
        builder
            .build()
            .unwrap_or_else(|_| panic!("missing variants"))
    }
}

impl<E: Enum, T: Clone> Clone for EnumMap<E, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: Enum, T: Clone> Copy for EnumMap<E, T> where GenericArray<T, E::Length>: Copy {}

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
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        E::variants().zip(self.0)
    }
}

#[macro_export]
macro_rules! enum_map {
    ($($t:tt)*) => {
        $crate::EnumMap::from_fn(|variant| match variant {
            $($t)*
        })
    };
}
