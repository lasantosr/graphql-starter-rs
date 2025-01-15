//! Allows the usage of [HashMap] in GraphQL with [GraphQLMap] by serializing it as a regular list of typed entries,
//! instead of the default opaque JSON.
//!
//! GraphQl [doesn't support](https://github.com/graphql/graphql-spec/issues/101) this.

use std::{
    borrow::Cow,
    collections::HashMap,
    fmt::{Debug, Display},
};

use async_graphql::{
    parser::types::Field, registry::Registry, resolver_utils::resolve_list, ContextSelectionSet, InputType,
    InputValueError, InputValueResult, OutputType, Positioned, ServerResult, Value,
};

use crate::error::{err, Error, GenericErrorCode, MapToErr};

/// This type represents a [HashMap] in GraphQL
#[derive(Debug, Clone, Default)]
pub struct GraphQLMap<E: GraphQLMapEntry>(Vec<E>);

/// Entry for a [GraphQLMap]
pub trait GraphQLMapEntry {
    type Key: Eq + std::hash::Hash;
    type Item;

    fn new(key: Self::Key, value: Self::Item) -> Self;
    fn into_parts(self) -> (Self::Key, Self::Item);
}

/// Creates a new entry type implementing [GraphQLMapEntry] for the given type.
///
/// ## Examples
///
/// ```
/// #[derive(Clone)]
/// pub struct MyType;
///
/// graphql_starter::map_entry_for!(
///     #[derive(Clone)]
///     MyType
/// );
/// ```
/// Expands to:
/// ``` ignore
/// #[derive(Clone)]
/// pub struct MyTypeEntry {
///     pub key: String,
///     pub value: MyType,
/// }
/// ```
///
/// You can also configure the key type or the entry type name:
/// ```
/// # use graphql_starter::map_entry_for;
/// # pub struct MyType {
/// #    value: String,
/// # }
/// map_entry_for!(MyType { key = u32 });
/// map_entry_for!(MyType { entry = MyCustomTypeEntry });
/// map_entry_for!(MyType { key = &'static str, entry = MyEntry });
/// ```
#[macro_export]
macro_rules! map_entry_for {
    ($(#[$attr:meta])* $name:ident) => {
        $crate::map_entry_for!{ $(#[$attr])* $name { key = String } }
    };

    ($(#[$attr:meta])* $name:ident { key = $key:ty }) => {
        $crate::crates::paste::paste! {
            $(#[$attr])*
            pub struct [<$name Entry>] {
                /// The entry key
                pub key: $key,
                /// The entry value
                pub value: $name,
            }
            impl $crate::graphql::GraphQLMapEntry for [<$name Entry>] {
                type Key = $key;
                type Item = $name;

                fn new(key: Self::Key, value: Self::Item) -> Self {
                    [<$name Entry>] { key, value }
                }

                fn into_parts(self) -> (Self::Key, Self::Item) {
                    (self.key, self.value)
                }
            }
        }
    };

    ($(#[$attr:meta])* $name:ident { entry = $entry:ident }) => {
        $crate::map_entry_for!{ $(#[$attr])* $name { entry = $entry, key = String } }
    };

    ($(#[$attr:meta])* $name:ident { key = $key:ty, entry = $entry:ident }) => {
        $crate::map_entry_for!{ $(#[$attr])* $name { entry = $entry, key = $key } }
    };

    ($(#[$attr:meta])* $name:ident { entry = $entry:ident, key = $key:ty }) => {
        $(#[$attr])*
        pub struct $entry {
            /// The entry key
            pub key: $key,
            /// The entry value
            pub value: $name,
        }
        impl $crate::graphql::GraphQLMapEntry for $entry {
            type Key = $key;
            type Item = $name;

            fn new(key: Self::Key, value: Self::Item) -> Self {
                $entry { key, value }
            }

            fn into_parts(self) -> (Self::Key, Self::Item) {
                (self.key, self.value)
            }
        }
    };
}

impl<E, K, T> From<HashMap<K, T>> for GraphQLMap<E>
where
    E: GraphQLMapEntry,
    K: Into<<E as GraphQLMapEntry>::Key>,
    T: Into<<E as GraphQLMapEntry>::Item>,
{
    fn from(map: HashMap<K, T>) -> Self {
        GraphQLMap(map.into_iter().map(|(k, v)| E::new(k.into(), v.into())).collect())
    }
}

impl<E> GraphQLMap<E>
where
    E: GraphQLMapEntry,
{
    /// Try to build a new [GraphQLMap] from a [HashMap]
    pub fn try_from<K, V>(map: HashMap<K, V>) -> Result<Self, Box<Error>>
    where
        K: Eq + std::hash::Hash + Display,
        K: TryInto<<E as GraphQLMapEntry>::Key>,
        <K as TryInto<<E as GraphQLMapEntry>::Key>>::Error: Display + Send + Sync + 'static,
        V: TryInto<<E as GraphQLMapEntry>::Item>,
        <V as TryInto<<E as GraphQLMapEntry>::Item>>::Error: Display + Send + Sync + 'static,
    {
        let mut vec = Vec::with_capacity(map.len());
        for (key, value) in map.into_iter() {
            let key = key.try_into().map_to_internal_err("Invalid map key")?;
            let value = value.try_into().map_to_internal_err("Invalid map value")?;

            vec.push(E::new(key, value));
        }
        Ok(GraphQLMap(vec))
    }
}

impl<E, K, V> TryFrom<GraphQLMap<E>> for HashMap<K, V>
where
    E: GraphQLMapEntry,
    K: Eq + std::hash::Hash + Display,
    <E as GraphQLMapEntry>::Key: TryInto<K>,
    <<E as GraphQLMapEntry>::Key as TryInto<K>>::Error: Display + Send + Sync + 'static,
    <E as GraphQLMapEntry>::Item: TryInto<V>,
    <<E as GraphQLMapEntry>::Item as TryInto<V>>::Error: Display + Send + Sync + 'static,
{
    type Error = Box<Error>;

    fn try_from(value: GraphQLMap<E>) -> Result<Self, Self::Error> {
        let mut map = HashMap::<K, V>::with_capacity(value.0.len());
        for e in value.0.into_iter() {
            let (key, value) = e.into_parts();

            let key = key
                .try_into()
                .map_to_err_with(GenericErrorCode::BadRequest, "Invalid map key")?;
            let value = value
                .try_into()
                .map_to_err_with(GenericErrorCode::BadRequest, "Invalid map value")?;

            #[allow(clippy::map_entry)] // If we insert first, we no longer have the key to generate the error message
            if map.contains_key(&key) {
                return Err(err!(GenericErrorCode::BadRequest, "Duplicated key: {}", key));
            } else {
                map.insert(key, value);
            }
        }
        Ok(map)
    }
}

impl<T: OutputType + GraphQLMapEntry> OutputType for GraphQLMap<T> {
    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("[{}]", T::qualified_type_name()))
    }

    fn qualified_type_name() -> String {
        format!("[{}]!", T::qualified_type_name())
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry);
        Self::qualified_type_name()
    }

    async fn resolve(&self, ctx: &ContextSelectionSet<'_>, field: &Positioned<Field>) -> ServerResult<Value> {
        resolve_list(ctx, field, &self.0, Some(self.0.len())).await
    }
}
impl<T: InputType + GraphQLMapEntry> InputType for GraphQLMap<T> {
    type RawValueType = Self;

    fn type_name() -> Cow<'static, str> {
        Cow::Owned(format!("[{}]", T::qualified_type_name()))
    }

    fn qualified_type_name() -> String {
        format!("[{}]!", T::qualified_type_name())
    }

    fn create_type_info(registry: &mut Registry) -> String {
        T::create_type_info(registry);
        Self::qualified_type_name()
    }

    fn parse(value: Option<Value>) -> InputValueResult<Self> {
        match value.unwrap_or_default() {
            Value::List(values) => {
                let list: Vec<_> = values
                    .into_iter()
                    .map(|value| InputType::parse(Some(value)))
                    .collect::<Result<_, _>>()
                    .map_err(InputValueError::propagate)?;

                Ok(GraphQLMap(list))
            }
            value => {
                let list = vec![InputType::parse(Some(value)).map_err(InputValueError::propagate)?];
                Ok(GraphQLMap(list))
            }
        }
    }

    fn to_value(&self) -> Value {
        Value::List(self.0.iter().map(InputType::to_value).collect())
    }

    fn as_raw_value(&self) -> Option<&Self::RawValueType> {
        Some(self)
    }
}
