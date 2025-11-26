//! Yuuka interoperability module
//!
//! This module provides utilities and examples for using hifumi together with yuuka.
//!
//! ## Approach
//!
//! Since yuuka uses procedural macros (`derive_struct!`) while hifumi uses attribute macros
//! (`#[version]`), deep integration is challenging. Instead, we provide:
//!
//! 1. **Serde-based interop**: Structures can be converted between yuuka and hifumi via JSON serialization
//! 2. **Migration helpers**: Utility functions to help migrate data between the two formats
//! 3. **Type compatibility**: Guidelines for designing types that work with both libraries
//!
//! ## Example
//!
//! ```rust,ignore
//! use yuuka::derive_struct;
//! use hifumi::version;
//! use serde::{Serialize, Deserialize};
//!
//! // Define a yuuka config structure
//! derive_struct!(
//!     #[derive(Serialize, Deserialize)]
//!     pub YuukaConfig {
//!         name: String,
//!         value: i32,
//!     }
//! );
//!
//! // Define a versioned hifumi structure with the same fields
//! #[version("0.1")]
//! #[derive(Debug, Clone, PartialEq)]
//! struct HifumiConfig {
//!     name: String,
//!     value: i32,
//! }
//!
//! // They can be converted via serde
//! let yuuka_cfg = YuukaConfig { name: "test".into(), value: 42 };
//! let json = serde_json::to_string(&yuuka_cfg).unwrap();
//!
//! // Note: hifumi adds $version field, so direct conversion needs care
//! ```

use serde::{de::DeserializeOwned, Serialize};

/// Convert a yuuka structure to a hifumi versioned structure via JSON.
///
/// This function serializes the yuuka struct to JSON and deserializes it back
/// as a hifumi versioned struct. Note that hifumi expects a `$version` field,
/// so this may require adding it manually.
///
/// # Example
/// ```ignore
/// let yuuka_data = YuukaConfig { name: "test".into(), value: 42 };
/// let hifumi_data: HifumiConfig = yuuka_to_hifumi_with_version(&yuuka_data, "0.1")?;
/// ```
pub fn yuuka_to_hifumi_with_version<T, U>(yuuka_data: &T, version: &str) -> anyhow::Result<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    // Serialize yuuka data to JSON value
    let mut json_value = serde_json::to_value(yuuka_data)?;

    // Add version field
    if let serde_json::Value::Object(ref mut map) = json_value {
        map.insert(
            "$version".to_string(),
            serde_json::Value::String(version.to_string()),
        );
    }

    // Deserialize as hifumi versioned struct
    Ok(serde_json::from_value(json_value)?)
}

/// Convert a hifumi versioned structure to a yuuka structure via JSON.
///
/// This function serializes the hifumi struct to JSON, removes the `$version`
/// field, and deserializes it as a yuuka struct.
///
/// # Example
/// ```ignore
/// let hifumi_data = HifumiConfig { name: "test".into(), value: 42 };
/// let yuuka_data: YuukaConfig = hifumi_to_yuuka(&hifumi_data)?;
/// ```
pub fn hifumi_to_yuuka<T, U>(hifumi_data: &T) -> anyhow::Result<U>
where
    T: Serialize,
    U: DeserializeOwned,
{
    // Serialize hifumi data to JSON value
    let mut json_value = serde_json::to_value(hifumi_data)?;

    // Remove version field
    if let serde_json::Value::Object(ref mut map) = json_value {
        map.remove("$version");
    }

    // Deserialize as yuuka struct
    Ok(serde_json::from_value(json_value)?)
}

/// Get the version string from a hifumi JSON representation.
///
/// # Example
/// ```ignore
/// let json = r#"{"$version":"0.2","name":"test","value":42}"#;
/// let version = get_version_from_json(json)?;
/// assert_eq!(version, Some("0.2".to_string()));
/// ```
pub fn get_version_from_json(json: &str) -> anyhow::Result<Option<String>> {
    let value: serde_json::Value = serde_json::from_str(json)?;
    Ok(value
        .get("$version")
        .and_then(|v| v.as_str())
        .map(String::from))
}

#[cfg(test)]
mod tests {
    use super::*;
    use hifumi::version;
    use serde::{Deserialize, Serialize};
    use yuuka::derive_struct;

    // Define a simple yuuka config
    // Note: yuuka automatically derives Debug and Clone, so we don't include them
    derive_struct!(
        #[derive(Serialize, Deserialize, PartialEq)]
        pub SimpleYuukaConfig {
            name: String,
            value: i32,
        }
    );

    // Define a hifumi versioned config with the same fields
    #[version("0.1")]
    #[derive(Debug, Clone, PartialEq)]
    struct SimpleHifumiConfig {
        name: String,
        value: i32,
    }

    #[test]
    fn test_yuuka_to_hifumi_conversion() {
        let yuuka_cfg = SimpleYuukaConfig {
            name: "test".to_string(),
            value: 42,
        };

        let hifumi_cfg: SimpleHifumiConfig =
            yuuka_to_hifumi_with_version(&yuuka_cfg, "0.1").unwrap();

        assert_eq!(hifumi_cfg.name, "test");
        assert_eq!(hifumi_cfg.value, 42);
    }

    #[test]
    fn test_hifumi_to_yuuka_conversion() {
        let hifumi_cfg = SimpleHifumiConfig {
            name: "hello".to_string(),
            value: 100,
        };

        let yuuka_cfg: SimpleYuukaConfig = hifumi_to_yuuka(&hifumi_cfg).unwrap();

        assert_eq!(yuuka_cfg.name, "hello");
        assert_eq!(yuuka_cfg.value, 100);
    }

    #[test]
    fn test_roundtrip_yuuka_hifumi_yuuka() {
        let original = SimpleYuukaConfig {
            name: "roundtrip".to_string(),
            value: 999,
        };

        // yuuka -> hifumi
        let hifumi: SimpleHifumiConfig = yuuka_to_hifumi_with_version(&original, "0.1").unwrap();

        // hifumi -> yuuka
        let result: SimpleYuukaConfig = hifumi_to_yuuka(&hifumi).unwrap();

        assert_eq!(original, result);
    }

    #[test]
    fn test_get_version_from_json() {
        let json = r#"{"$version":"0.2","name":"test","value":42}"#;
        let version = get_version_from_json(json).unwrap();
        assert_eq!(version, Some("0.2".to_string()));

        let json_no_version = r#"{"name":"test","value":42}"#;
        let no_version = get_version_from_json(json_no_version).unwrap();
        assert_eq!(no_version, None);
    }
}
