use super::{Enum, EnumMap};
use std::{
    mem::{self, ManuallyDrop, MaybeUninit},
    ptr,
};

pub struct Builder<E: Enum, T> {
    uninit: EnumMap<E, MaybeUninit<T>>,
    pub(super) mut(self) is_init: EnumMap<E, bool>,
    count: usize,
}

impl<E: Enum, T> Builder<E, T> {
    pub(super) fn init(&mut self, variant: E, value: T) -> bool {
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

impl<E: Enum, T> Default for Builder<E, T> {
    fn default() -> Self {
        Self {
            uninit: EnumMap::uninit(),
            is_init: Default::default(),
            count: 0,
        }
    }
}

impl<E: Enum, T> Drop for Builder<E, T> {
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
