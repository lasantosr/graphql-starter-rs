/// Like an [Option] but with an additional [Unset](MaybeOption::Unset) variant, that's why it's maybe an option.
///
/// This type is useful to disambiguate between absent values and those set explicitly to none/null.
///
/// # Serde
///
/// Serializing requires to skip unset variants, and deserializing requires default:
///
/// ```ignore
/// #[serde(default, skip_serializing_if = "MaybeOption::is_unset")]
/// ```
#[derive(Default, Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum MaybeOption<T> {
    #[default]
    Unset,
    None,
    Some(T),
}

impl<T> MaybeOption<T> {
    /// Returns `true` if the option is an [`Unset`](MaybeOption::Unset) value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let x: MaybeOption<u32> = MaybeOption::Unset;
    /// assert_eq!(x.is_unset(), true);
    ///
    /// let x: MaybeOption<u32> = MaybeOption::None;
    /// assert_eq!(x.is_unset(), false);
    /// ```
    #[inline]
    pub const fn is_unset(&self) -> bool {
        matches!(*self, Self::Unset)
    }

    /// Returns `true` if the option is a [`None`](MaybeOption::None) value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let x: MaybeOption<u32> = MaybeOption::Some(2);
    /// assert_eq!(x.is_none(), false);
    ///
    /// let x: MaybeOption<u32> = MaybeOption::None;
    /// assert_eq!(x.is_none(), true);
    /// ```
    #[inline]
    pub const fn is_none(&self) -> bool {
        matches!(*self, Self::None)
    }

    /// Returns `true` if the option is a [`Some`](MaybeOption::Some) value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let x: MaybeOption<u32> = MaybeOption::Some(2);
    /// assert_eq!(x.is_some(), true);
    ///
    /// let x: MaybeOption<u32> = MaybeOption::None;
    /// assert_eq!(x.is_some(), false);
    /// ```
    #[inline]
    pub const fn is_some(&self) -> bool {
        matches!(*self, Self::Some(_))
    }

    /// Converts from `&MaybeOption<T>` to `MaybeOption<&T>`.
    ///
    /// # Examples
    ///
    /// Calculates the length of an <code>MaybeOption<[String]></code> as an <code>MaybeOption<[usize]></code>
    /// without moving the [`String`]. The [`map`] method takes the `self` argument by value,
    /// consuming the original, so this technique uses `as_ref` to first take an `MaybeOption` to a
    /// reference to the value inside the original.
    ///
    /// [`map`]: MaybeOption::map
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let text: MaybeOption<String> = MaybeOption::Some("Hello, world!".to_string());
    /// // First, cast `Option<String>` to `Option<&String>` with `as_ref`,
    /// // then consume *that* with `map`, leaving `text` on the stack.
    /// let text_length: MaybeOption<usize> = text.as_ref().map(|s| s.len());
    /// println!("still can print text: {text:?}");
    /// ```
    #[inline]
    pub const fn as_ref(&self) -> MaybeOption<&T> {
        match *self {
            MaybeOption::Some(ref x) => MaybeOption::Some(x),
            MaybeOption::None => MaybeOption::None,
            MaybeOption::Unset => MaybeOption::Unset,
        }
    }

    /// Converts from `&mut MaybeOption<T>` to `MaybeOption<&mut T>`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let mut x = MaybeOption::Some(2);
    /// match x.as_mut() {
    ///     MaybeOption::Some(v) => *v = 42,
    ///     MaybeOption::None => {}
    ///     MaybeOption::Unset => {}
    /// }
    /// assert_eq!(x, MaybeOption::Some(42));
    /// ```
    #[inline]
    pub fn as_mut(&mut self) -> MaybeOption<&mut T> {
        match *self {
            MaybeOption::Some(ref mut x) => MaybeOption::Some(x),
            MaybeOption::None => MaybeOption::None,
            MaybeOption::Unset => MaybeOption::Unset,
        }
    }

    /// Maps a `MaybeOption<T>` to `MaybeOption<U>` by applying a function to a contained value (if `Some`).
    ///
    /// # Examples
    ///
    /// Calculates the length of an <code>MaybeOption<[String]></code> as an
    /// <code>MaybeOption<[usize]></code>, consuming the original:
    ///
    /// ```
    /// # use graphql_starter::utils::MaybeOption;
    /// let maybe_some_string = MaybeOption::Some(String::from("Hello, World!"));
    /// // `Option::map` takes self *by value*, consuming `maybe_some_string`
    /// let maybe_some_len = maybe_some_string.map(|s| s.len());
    /// assert_eq!(maybe_some_len, MaybeOption::Some(13));
    ///
    /// let x: MaybeOption<&str> = MaybeOption::None;
    /// assert_eq!(x.map(|s| s.len()), MaybeOption::None);
    ///
    /// let x: MaybeOption<&str> = MaybeOption::Unset;
    /// assert_eq!(x.map(|s| s.len()), MaybeOption::Unset);
    /// ```
    #[inline]
    pub fn map<U, F>(self, f: F) -> MaybeOption<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            MaybeOption::Some(x) => MaybeOption::Some(f(x)),
            MaybeOption::None => MaybeOption::None,
            MaybeOption::Unset => MaybeOption::Unset,
        }
    }

    /// Maps a `MaybeOption<T>` to `MaybeOption<U>` by applying a function
    /// to the contained nullable value
    #[inline]
    pub fn map_option<U, F: FnOnce(Option<T>) -> Option<U>>(self, f: F) -> MaybeOption<U> {
        match self {
            MaybeOption::Some(v) => match f(Some(v)) {
                Some(v) => MaybeOption::Some(v),
                None => MaybeOption::None,
            },
            MaybeOption::None => match f(None) {
                Some(v) => MaybeOption::Some(v),
                None => MaybeOption::None,
            },
            MaybeOption::Unset => MaybeOption::Unset,
        }
    }

    /// Converts this value into the double-option pattern
    #[inline]
    pub fn into_double_option(self) -> Option<Option<T>> {
        self.into()
    }
}
impl<T, E> MaybeOption<Result<T, E>> {
    /// Transposes a `MaybeOption` of a [`Result`] into a [`Result`] of a
    /// `MaybeOption`.
    #[inline]
    pub fn transpose(self) -> Result<MaybeOption<T>, E> {
        match self {
            MaybeOption::Unset => Ok(MaybeOption::Unset),
            MaybeOption::None => Ok(MaybeOption::None),
            MaybeOption::Some(Ok(v)) => Ok(MaybeOption::Some(v)),
            MaybeOption::Some(Err(e)) => Err(e),
        }
    }
}

impl<T, U> From<MaybeOption<T>> for Option<Option<U>>
where
    U: From<T>,
{
    fn from(value: MaybeOption<T>) -> Self {
        match value {
            MaybeOption::Unset => None,
            MaybeOption::None => Some(None),
            MaybeOption::Some(t) => Some(Some(U::from(t))),
        }
    }
}
impl<T, U> From<Option<Option<T>>> for MaybeOption<U>
where
    U: From<T>,
{
    fn from(value: Option<Option<T>>) -> Self {
        match value {
            None => Self::Unset,
            Some(None) => Self::None,
            Some(Some(t)) => Self::Some(U::from(t)),
        }
    }
}

#[cfg(feature = "graphql")]
pub mod graphql {
    use std::borrow::Cow;

    use async_graphql::{registry, InputType, InputValueError, InputValueResult, MaybeUndefined, Value};

    use super::*;

    impl<T> From<MaybeOption<T>> for MaybeUndefined<T> {
        fn from(value: MaybeOption<T>) -> Self {
            match value {
                MaybeOption::Unset => Self::Undefined,
                MaybeOption::None => Self::Null,
                MaybeOption::Some(t) => Self::Value(t),
            }
        }
    }
    impl<T> From<MaybeUndefined<T>> for MaybeOption<T> {
        fn from(value: MaybeUndefined<T>) -> Self {
            match value {
                MaybeUndefined::Undefined => Self::Unset,
                MaybeUndefined::Null => Self::None,
                MaybeUndefined::Value(t) => Self::Some(t),
            }
        }
    }

    impl<T: InputType> InputType for MaybeOption<T> {
        type RawValueType = T::RawValueType;

        fn type_name() -> Cow<'static, str> {
            T::type_name()
        }

        fn qualified_type_name() -> String {
            T::type_name().to_string()
        }

        fn create_type_info(registry: &mut registry::Registry) -> String {
            T::create_type_info(registry);
            T::type_name().to_string()
        }

        fn parse(value: Option<Value>) -> InputValueResult<Self> {
            match value {
                None => Ok(MaybeOption::Unset),
                Some(Value::Null) => Ok(MaybeOption::None),
                Some(value) => Ok(MaybeOption::Some(
                    T::parse(Some(value)).map_err(InputValueError::propagate)?,
                )),
            }
        }

        fn to_value(&self) -> Value {
            match self {
                MaybeOption::Some(value) => value.to_value(),
                _ => Value::Null,
            }
        }

        fn as_raw_value(&self) -> Option<&Self::RawValueType> {
            if let MaybeOption::Some(value) = self {
                value.as_raw_value()
            } else {
                None
            }
        }
    }
}

#[cfg(feature = "model-mapper")]
pub mod mapper {
    use model_mapper::with::{NestedWrapper, Wrapper};

    use super::*;

    impl<T> Wrapper<T> for MaybeOption<T> {
        type Wrapper<U> = MaybeOption<U>;

        fn map_inner<Z: Fn(T) -> U, U>(self, f: Z) -> Self::Wrapper<U> {
            self.map(f)
        }

        fn try_map_inner<Z: Fn(T) -> Result<U, E>, U, E>(self, f: Z) -> Result<Self::Wrapper<U>, E> {
            self.map(f).transpose()
        }
    }

    impl<W: Wrapper<T>, T> NestedWrapper<W, T> for MaybeOption<W> {
        type NestedWrapper<U> = MaybeOption<W::Wrapper<U>>;

        fn map_wrapper<Z: Fn(T) -> U, U>(self, f: Z) -> Self::NestedWrapper<U> {
            self.map(|i| i.map_inner(f))
        }

        fn try_map_wrapper<Z: Fn(T) -> Result<U, E>, U, E>(self, f: Z) -> Result<Self::NestedWrapper<U>, E> {
            self.map(|i| i.try_map_inner(f)).transpose()
        }
    }
}

#[cfg(feature = "garde")]
pub mod garde {
    use ::garde::{error::NoKey, rules::inner::Inner};

    use super::*;

    impl<T> Inner<T> for MaybeOption<T> {
        type Key = NoKey;

        fn validate_inner<F>(&self, mut f: F)
        where
            F: FnMut(&T, &Self::Key),
        {
            if let MaybeOption::Some(item) = self {
                f(item, &NoKey::default())
            }
        }
    }
}

pub mod serde {
    use std::marker::PhantomData;

    use ::serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::*;

    impl<T> Serialize for MaybeOption<T>
    where
        T: Serialize,
    {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            use ::serde::ser::Error;
            match *self {
                Self::Unset => Err(S::Error::custom(
                    "can't serialize an unset value, please include skip_serializing_if",
                )),
                Self::None => serializer.serialize_none(),
                Self::Some(ref value) => value.serialize(serializer),
            }
        }
    }

    pub struct MaybeOptionVisitor<T> {
        marker: PhantomData<T>,
    }

    impl<'de, T> Deserialize<'de> for MaybeOption<T>
    where
        T: Deserialize<'de>,
    {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            deserializer.deserialize_option(MaybeOptionVisitor::<T> { marker: PhantomData })
        }
    }

    impl<'de, T> ::serde::de::Visitor<'de> for MaybeOptionVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = MaybeOption<T>;

        #[inline]
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("either a missing field, a field with null/None, or a field with value T")
        }

        #[inline]
        fn visit_unit<E>(self) -> Result<MaybeOption<T>, E>
        where
            E: ::serde::de::Error,
        {
            Ok(MaybeOption::Unset)
        }

        #[inline]
        fn visit_none<E>(self) -> Result<MaybeOption<T>, E>
        where
            E: ::serde::de::Error,
        {
            Ok(MaybeOption::None)
        }

        #[inline]
        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            T::deserialize(deserializer).map(MaybeOption::Some)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_unset_fails() {
        #[derive(::serde::Serialize)]
        struct A {
            foo: MaybeOption<usize>,
        }

        let a = A {
            foo: MaybeOption::Unset,
        };

        let res = serde_json::to_string(&a);
        assert!(res.is_err());
    }

    #[test]
    fn test_serialize_succeeds() {
        #[derive(::serde::Serialize)]
        struct A {
            #[serde(skip_serializing_if = "MaybeOption::is_unset")]
            unset: MaybeOption<usize>,
            #[serde(skip_serializing_if = "MaybeOption::is_unset")]
            none: MaybeOption<usize>,
            #[serde(skip_serializing_if = "MaybeOption::is_unset")]
            set: MaybeOption<usize>,
        }

        let a = A {
            unset: MaybeOption::Unset,
            none: MaybeOption::None,
            set: MaybeOption::Some(1),
        };

        let res = serde_json::to_string(&a);
        assert!(res.is_ok());
        assert_eq!(r#"{"none":null,"set":1}"#, res.unwrap());
    }

    #[test]
    fn test_deserialize_succeeds() {
        #[derive(::serde::Deserialize, Debug, PartialEq, Eq)]
        #[serde(deny_unknown_fields)]
        struct A {
            #[serde(default)]
            unset: MaybeOption<usize>,
            #[serde(default)]
            none: MaybeOption<usize>,
            #[serde(default)]
            set: MaybeOption<usize>,
        }

        let res = serde_json::from_str::<A>(r#"{"none":null,"set":1}"#);
        assert!(res.is_ok(), "Expected to succeed: {res:?}");
        assert_eq!(
            A {
                unset: MaybeOption::Unset,
                none: MaybeOption::None,
                set: MaybeOption::Some(1),
            },
            res.unwrap()
        );
    }
}
