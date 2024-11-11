//! Read config from toml and env variables using [Figment]

use std::{env, path::Path};

use anyhow::{Context, Result};
use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::de::DeserializeOwned;

/// Reads the config folder into a [Figment]
///
/// The order of precedence for properties is:
/// - Environment variables, splitting objects by `_`
/// - `{profile}.toml`, where `{profile}` is retrieved from `PROFILE` environment variable and defaults to `development`
/// - `default.toml`
///
/// **IMPORTANT:** Properties names can't contain underscores, as they're reserved to split nested objects.
pub fn read(path: impl AsRef<Path>) -> Figment {
    let path = path.as_ref();
    let profile = env::var("PROFILE");
    let profile = profile.as_deref().unwrap_or("development");

    Figment::new()
        // Load defaults
        .merge(Toml::file(path.join("default.toml")))
        // Load profile overrides
        .merge(Toml::file(path.join(format!("{profile}.toml"))))
        // Load environment variables
        .merge(Env::raw().split("_"))
}

/// Reads the config and parses it into the given `T`
///
/// The order of precedence for properties is:
/// - Environment variables, splitting objects by `_`
/// - `{profile}.toml`, where `{profile}` is retrieved from `PROFILE` environment variable and defaults to `development`
/// - `default.toml`
///
/// **IMPORTANT:** Properties names can't contain underscores, as they're reserved to split nested objects. `T` can
/// still contain underscored properties if they have an alias when deserializing:
///
/// ``` ignore
/// #[derive(Deserialize)]
/// struct Config {
///     name: String,
///     #[serde(alias = "nameprefix")]
///     name_prefix: String,
/// }
/// ```
pub fn parse<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    read(path).extract().context("Could not parse config")
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Deserialize)]
    struct Config {
        name: String,
        port: u16,
        child: ChildConfig,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct ChildConfig {
        #[serde(rename = "nameprefix")]
        name_prefix: String,
    }

    #[test]
    fn test_config() {
        figment::Jail::expect_with(|jail| {
            let tmp_dir = jail.directory();
            fs::create_dir(tmp_dir.join("config")).unwrap();
            jail.create_file(
                "config/default.toml",
                r#"
                    name = "test"
                    port = 8080
                    
                    [child]
                    nameprefix = "test-prefix"
                "#,
            )?;
            jail.create_file(
                "config/development.toml",
                r#"
                    port = 8081
                "#,
            )?;

            jail.set_env("NAME", "env-test");
            jail.set_env("CHILD_NAMEPREFIX", "env-test-prefix");

            let figment = read("config");
            assert_eq!("env-test".to_string(), figment.extract_inner::<String>("name")?);
            assert_eq!(8081, figment.extract_inner::<u16>("port")?);
            assert_eq!(
                "env-test-prefix".to_string(),
                figment.extract_inner::<String>("child.nameprefix")?
            );

            assert_eq!(
                figment.extract::<Config>()?,
                Config {
                    name: "env-test".into(),
                    port: 8081,
                    child: ChildConfig {
                        name_prefix: "env-test-prefix".into()
                    }
                }
            );

            Ok(())
        });
    }
}
