use ::chrono::{Duration, FixedOffset};
use ::serde::{de::Error, Deserialize, Deserializer, Serializer};

/// De/serialize a chrono [Duration] in/to minutes
pub mod duration_mins {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let minutes: i64 = Deserialize::deserialize(d)?;
        Ok(Duration::minutes(minutes))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(d.num_minutes())
    }
}

/// De/serialize an optional chrono [Duration] in/to minutes
pub mod duration_mins_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let minutes: Option<i64> = Deserialize::deserialize(d)?;
        Ok(minutes.map(Duration::minutes))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.num_minutes()),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize a chrono [Duration] in/to seconds
pub mod duration_secs {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: i64 = Deserialize::deserialize(d)?;
        Ok(Duration::seconds(seconds))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(d.num_seconds())
    }
}

/// De/serialize an optional chrono [Duration] in/to seconds
pub mod duration_secs_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: Option<i64> = Deserialize::deserialize(d)?;
        Ok(seconds.map(Duration::seconds))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.num_seconds()),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize a chrono [Duration] in/to milliseconds
pub mod duration_millis {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: i64 = Deserialize::deserialize(d)?;
        Ok(Duration::milliseconds(millis))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i64(d.num_milliseconds())
    }
}

/// De/serialize an optional chrono [Duration] in/to milliseconds
pub mod duration_millis_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: Option<i64> = Deserialize::deserialize(d)?;
        Ok(millis.map(Duration::milliseconds))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.num_milliseconds()),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize a chrono [FixedOffset] in/to seconds
pub mod offset_secs {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<FixedOffset, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: i32 = Deserialize::deserialize(d)?;
        FixedOffset::east_opt(seconds).ok_or_else(|| D::Error::custom("out of bounds"))
    }

    pub fn serialize<S>(d: &FixedOffset, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_i32(d.local_minus_utc())
    }
}

/// De/serialize an optional chrono [FixedOffset] in/to seconds
pub mod offset_secs_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<FixedOffset>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: Option<i32> = Deserialize::deserialize(d)?;
        seconds
            .map(|s| FixedOffset::east_opt(s).ok_or_else(|| D::Error::custom("out of bounds")))
            .transpose()
    }

    pub fn serialize<S>(opt: &Option<FixedOffset>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.local_minus_utc()),
            None => serializer.serialize_none(),
        }
    }
}
