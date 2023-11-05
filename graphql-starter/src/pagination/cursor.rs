use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};
use serde::{
    de::{DeserializeOwned, Error as SerdeError},
    Deserialize, Deserializer, Serialize, Serializer,
};

use super::PaginationErrorCode;
use crate::error::{MapToErr, Result};

/// Opaque cursor
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpaqueCursor(Vec<u8>);
impl OpaqueCursor {
    /// Decodes the given base64 string into an [OpaqueCursor]
    pub fn decode(cursor: impl AsRef<str>) -> Result<Self> {
        let data = BASE64_URL_SAFE_NO_PAD.decode(cursor.as_ref()).map_to_err(
            PaginationErrorCode::PageInvalidCursor,
            "Couldn't decode the cursor as base64",
        )?;

        Ok(Self(data))
    }

    /// Encodes this [OpaqueCursor] into a base64 string
    pub fn encode(&self) -> String {
        BASE64_URL_SAFE_NO_PAD.encode(&self.0)
    }

    /// Serializes any data into an [OpaqueCursor]
    pub fn new<T>(data: &T) -> Result<Self>
    where
        T: Serialize,
    {
        Ok(Self(
            serde_json::to_vec(data).map_to_internal_err("Couldn't serialize a cursor")?,
        ))
    }

    /// Deserializes the [OpaqueCursor] into the given data type
    pub fn as_data<T>(&self) -> Result<T>
    where
        T: DeserializeOwned,
    {
        serde_json::from_slice(&self.0).map_to_err(
            PaginationErrorCode::PageInvalidCursor,
            "Couldn't deserialize the cursor into the expected type",
        )
    }
}
impl Serialize for OpaqueCursor {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.encode())
    }
}
impl<'de> Deserialize<'de> for OpaqueCursor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let cursor: String = serde::Deserialize::deserialize(deserializer)?;
        Self::decode(cursor).map_err(|err| D::Error::custom(err.info().message()))
    }
}
