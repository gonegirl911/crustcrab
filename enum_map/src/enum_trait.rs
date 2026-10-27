pub use enum_derive::Enum;

use generic_array::{
    ArrayLength,
    typenum::{Add1, B1, U2, Unsigned},
};
use std::ops::Add;

#[expect(clippy::missing_safety_doc)]
pub unsafe trait Enum: Copy {
    type Length: ArrayLength;

    const LEN: usize = Self::Length::USIZE;

    fn from_index(index: usize) -> Option<Self> {
        (index < Self::LEN).then(|| unsafe { Self::from_index_unchecked(index) })
    }

    unsafe fn from_index_unchecked(index: usize) -> Self;

    fn to_index(self) -> usize;

    #[define_opaque(Variants)]
    fn variants() -> Variants<Self> {
        (0..Self::LEN).map(|i| unsafe { Self::from_index_unchecked(i) })
    }
}

pub type Variants<E: Enum> = impl Iterator<Item = E>;

unsafe impl Enum for bool {
    type Length = U2;

    unsafe fn from_index_unchecked(index: usize) -> Self {
        index != 0
    }

    fn to_index(self) -> usize {
        self as usize
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
