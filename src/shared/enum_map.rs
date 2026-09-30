use generic_array::{
    ArrayLength, GenericArray, GenericArrayIter,
    functional::FunctionalSequence,
    sequence::GenericSequence,
    typenum::{Add1, Unsigned, bit::B1},
};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    fmt::{self, Display, Formatter},
    marker::PhantomData,
    mem::{self, ManuallyDrop, MaybeUninit},
    ops::{Add, Deref, Index, IndexMut},
    ptr, slice,
};

pub use macros::Enum;

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

    fn builder() -> EnumMapBuilder<E, T> {
        EnumMapBuilder::default()
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

    pub fn inner(&self) -> &GenericArray<T, E::Length> {
        &self.0
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
}

impl<E: Enum, T: Deref> EnumMap<E, T> {
    pub fn each_deref(&self) -> EnumMap<E, &T::Target> {
        EnumMap(self.0.each_ref().map(Deref::deref))
    }
}

impl<E: Enum, T> EnumMap<E, MaybeUninit<T>> {
    unsafe fn assume_init(self) -> EnumMap<E, T> {
        EnumMap(unsafe { GenericArray::assume_init(self.0) })
    }
}

impl<E: Enum, T: Clone> Clone for EnumMap<E, T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E: Enum, T: Clone> Copy for EnumMap<E, T> where GenericArray<T, E::Length>: Copy {}

impl<E: Enum, T: PartialEq> PartialEq for EnumMap<E, T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
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

impl<'de, E, T> Deserialize<'de> for EnumMap<E, T>
where
    E: Enum + Serialize + Deserialize<'de>,
    T: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MapVisitor<E: Enum, T>(PhantomData<fn() -> EnumMap<E, T>>);

        impl<'de, E, T> Visitor<'de> for MapVisitor<E, T>
        where
            E: Enum + Serialize + Deserialize<'de>,
            T: Deserialize<'de>,
        {
            type Value = EnumMap<E, T>;

            fn expecting(&self, f: &mut Formatter) -> fmt::Result {
                write!(f, "a map")
            }

            fn visit_map<M: MapAccess<'de>>(self, mut access: M) -> Result<Self::Value, M::Error> {
                let mut builder = EnumMap::builder();

                while let Some((variant, value)) = access.next_entry()? {
                    if !builder.init(variant, value) {
                        return Err(de::Error::custom(format_args!(
                            "duplicate variant \"{}\"",
                            SerializeDisplay(variant),
                        )));
                    }
                }

                builder.build().map_err(|builder| {
                    de::Error::custom(format_args!(
                        "missing variants [\"{}\"]",
                        MissingVariants(&builder.is_init),
                    ))
                })
            }
        }

        deserializer.deserialize_map(MapVisitor(PhantomData))
    }
}

struct SerializeDisplay<T>(T);

impl<T: Serialize> Display for SerializeDisplay<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.serialize(f)
    }
}

struct MissingVariants<'a, E: Enum>(&'a EnumMap<E, bool>);

impl<E: Enum + Serialize> Display for MissingVariants<'_, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .filter(|&(_, &is_init)| !is_init)
            .enumerate()
            .try_for_each(|(i, (variant, _))| {
                if i > 0 {
                    write!(f, "\", \"")?;
                }
                write!(f, "{}", SerializeDisplay(variant))
            })
    }
}

pub struct EnumMapBuilder<E: Enum, T> {
    uninit: EnumMap<E, MaybeUninit<T>>,
    is_init: EnumMap<E, bool>,
    count: usize,
}

impl<E: Enum, T> EnumMapBuilder<E, T> {
    fn init(&mut self, variant: E, value: T) -> bool {
        if self.is_init[variant] {
            false
        } else {
            self.set(variant, value);
            true
        }
    }

    pub fn set(&mut self, variant: E, value: T) -> Option<T> {
        let is_init = self.is_init[variant];
        let prev = is_init.then(|| unsafe { self.uninit[variant].assume_init_read() });
        self.uninit[variant].write(value);
        self.count += !is_init as usize;
        self.is_init[variant] = true;
        prev
    }

    pub fn build(self) -> Result<EnumMap<E, T>, Self> {
        if self.count == E::LEN {
            let this = ManuallyDrop::new(self);
            Ok(unsafe { ptr::read(&this.uninit).assume_init() })
        } else {
            Err(self)
        }
    }
}

impl<E: Enum, T> Default for EnumMapBuilder<E, T> {
    fn default() -> Self {
        Self {
            uninit: EnumMap::uninit(),
            is_init: Default::default(),
            count: 0,
        }
    }
}

impl<E: Enum, T> Drop for EnumMapBuilder<E, T> {
    fn drop(&mut self) {
        if !mem::needs_drop::<T>() {
            return;
        }

        for (uninit, &is_init) in self.uninit.values_mut().zip(self.is_init.values()) {
            if is_init {
                unsafe { uninit.assume_init_drop() };
            }
        }
    }
}

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
