use ::serde::{de::Error, Deserialize, Deserializer, Serializer};
use ::std::{str::FromStr, time::Duration};

const NANOS_PER_MILLI: u32 = 1_000_000;
const MILLIS_PER_SEC: u128 = 1_000;
const SECS_PER_MINUTE: u64 = 60;

/// De/serialize an std [Duration] in/to minutes
pub mod duration_mins {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let minutes: u64 = Deserialize::deserialize(d)?;
        Ok(Duration::from_secs(minutes * SECS_PER_MINUTE))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(d.as_secs() / SECS_PER_MINUTE)
    }
}

/// De/serialize an optional std [Duration] in/to minutes
pub mod duration_mins_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let minutes: Option<u64> = Deserialize::deserialize(d)?;
        Ok(minutes.map(|mins| Duration::from_secs(mins * SECS_PER_MINUTE)))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&(d.as_secs() / SECS_PER_MINUTE)),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize an std [Duration] in/to seconds
pub mod duration_secs {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: u64 = Deserialize::deserialize(d)?;
        Ok(Duration::from_secs(seconds))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(d.as_secs())
    }
}

/// De/serialize an optional std [Duration] in/to seconds
pub mod duration_secs_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds: Option<u64> = Deserialize::deserialize(d)?;
        Ok(seconds.map(Duration::from_secs))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.as_secs()),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize an std [Duration] in/to milliseconds
pub mod duration_millis {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: u128 = Deserialize::deserialize(d)?;
        Ok(Duration::new(
            (millis / MILLIS_PER_SEC) as u64,
            ((millis % MILLIS_PER_SEC) as u32) * NANOS_PER_MILLI,
        ))
    }

    pub fn serialize<S>(d: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u128(d.as_millis())
    }
}

/// De/serialize an optional std [Duration] in/to milliseconds
pub mod duration_millis_opt {

    use super::*;

    pub fn deserialize<'de, D>(d: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: Option<u128> = Deserialize::deserialize(d)?;
        Ok(millis.map(|m| {
            Duration::new(
                (m / MILLIS_PER_SEC) as u64,
                ((m % MILLIS_PER_SEC) as u32) * NANOS_PER_MILLI,
            )
        }))
    }

    pub fn serialize<S>(opt: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(d) => serializer.serialize_some(&d.as_millis()),
            None => serializer.serialize_none(),
        }
    }
}

/// De/serialize an [f64] in/to an [f64] or an [String] if it's `NaN` or `Inf`
pub mod f64 {

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<f64, D::Error>
    where
        D: Deserializer<'de>,
    {
        match serde_json::Value::deserialize(deserializer)? {
            serde_json::Value::String(s) => f64::from_str(&s).map_err(Error::custom),
            serde_json::Value::Number(s) => s
                .as_f64()
                .ok_or(Error::custom("could not convert the number to an f64")),
            _ => Err(Error::custom("expected string or number")),
        }
    }

    pub fn serialize<S>(val: &f64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if val.is_nan() {
            serializer.serialize_str("NaN")
        } else if *val == f64::INFINITY {
            serializer.serialize_str("+Inf")
        } else if *val == f64::NEG_INFINITY {
            serializer.serialize_str("-Inf")
        } else {
            serializer.serialize_f64(*val)
        }
    }
}

/// De/serialize an optional [f64] in/to an [f64] or an [String] if it's `NaN` or `Inf`
pub mod f64_opt {

    use super::*;

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<serde_json::Value>::deserialize(deserializer)? {
            None => Ok(None),
            Some(serde_json::Value::String(s)) => Ok(Some(f64::from_str(&s).map_err(Error::custom)?)),
            Some(serde_json::Value::Number(s)) => Ok(Some(
                s.as_f64()
                    .ok_or(Error::custom("could not convert the number to an f64"))?,
            )),
            _ => Err(Error::custom("expected string or number")),
        }
    }

    pub fn serialize<S>(opt: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *opt {
            Some(val) => {
                if val.is_nan() {
                    serializer.serialize_some("NaN")
                } else if val == f64::INFINITY {
                    serializer.serialize_some("+Inf")
                } else if val == f64::NEG_INFINITY {
                    serializer.serialize_some("-Inf")
                } else {
                    serializer.serialize_some(&val)
                }
            }
            None => serializer.serialize_none(),
        }
    }
}
