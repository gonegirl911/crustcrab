use super::{Enum, EnumMap};
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, MapAccess, Visitor},
};
use std::{
    fmt::{self, Display, Formatter},
    marker::PhantomData,
};

impl<'de, E, T> Deserialize<'de> for EnumMap<E, T>
where
    E: Enum + Serialize + Deserialize<'de>,
    T: Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        if deserializer.is_human_readable() {
            deserializer.deserialize_map(MapVisitor(PhantomData))
        } else {
            unreachable!()
        }
    }
}

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
