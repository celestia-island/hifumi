//! Complex migration scenarios test
//!
//! This test demonstrates complex versioning and migration scenarios
//! between hifumi and yuuka.

use anyhow::Result;
use hifumi::version;
use serde::{Deserialize, Serialize};
use yuuka::derive_struct;

use hifumi_e2e::yuuka_interop::{hifumi_to_yuuka, yuuka_to_hifumi_with_version};

// ============================================================================
// Scenario 1: Nested structures
// ============================================================================

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaAddress {
        street: String,
        city: String,
        country: String,
    }
);

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaUser {
        id: i32,
        name: String,
        address: YuukaAddress,
    }
);

#[version("0.2")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.1" => "0.2" {
    + country: String,
})]
struct HifumiAddress {
    street: String,
    city: String,
    country: String,
}

#[version("0.1")]
#[derive(Debug, Clone, PartialEq)]
struct HifumiUser {
    id: i32,
    name: String,
    address: HifumiAddress,
}

#[test]
fn test_nested_struct_conversion() -> Result<()> {
    let yuuka_user = YuukaUser {
        id: 1,
        name: "Alice".to_string(),
        address: YuukaAddress {
            street: "123 Main St".to_string(),
            city: "Tokyo".to_string(),
            country: "Japan".to_string(),
        },
    };

    // Convert to JSON and manually add versions
    let json = serde_json::to_string(&yuuka_user)?;

    // The nested structure conversion needs special handling
    // since hifumi adds $version to each level
    let mut json_value: serde_json::Value = serde_json::from_str(&json)?;

    // Add version to root
    if let serde_json::Value::Object(ref mut map) = json_value {
        map.insert(
            "$version".to_string(),
            serde_json::Value::String("0.1".to_string()),
        );

        // Add version to nested address
        if let Some(serde_json::Value::Object(ref mut addr_map)) = map.get_mut("address") {
            addr_map.insert(
                "$version".to_string(),
                serde_json::Value::String("0.2".to_string()),
            );
        }
    }

    let hifumi_user: HifumiUser = serde_json::from_value(json_value)?;

    assert_eq!(hifumi_user.id, 1);
    assert_eq!(hifumi_user.name, "Alice");
    assert_eq!(hifumi_user.address.city, "Tokyo");

    Ok(())
}

// ============================================================================
// Scenario 2: Version migration from old format
// ============================================================================

#[version("0.3")]
#[derive(Debug, Clone, PartialEq)]
#[migration("0.2" => "0.3" {
    + email: String,
})]
#[migration("0.1" => "0.2" {
    nickname => display_name: String,
})]
struct VersionedProfile {
    id: i32,
    display_name: String,
    email: String,
}

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaProfileV1 {
        id: i32,
        nickname: String,
    }
);

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaProfileV3 {
        id: i32,
        display_name: String,
        email: String,
    }
);

#[test]
fn test_migrate_yuuka_v1_to_hifumi_v3() -> Result<()> {
    // Old yuuka format (v1 equivalent)
    let old_profile = YuukaProfileV1 {
        id: 42,
        nickname: "OldNick".to_string(),
    };

    // Convert with v1 version tag - hifumi will migrate it
    let json = serde_json::to_value(&old_profile)?;
    let mut json_obj = json.as_object().unwrap().clone();
    json_obj.insert(
        "$version".to_string(),
        serde_json::Value::String("0.1".to_string()),
    );

    let migrated: VersionedProfile = serde_json::from_value(serde_json::Value::Object(json_obj))?;

    assert_eq!(migrated.id, 42);
    assert_eq!(migrated.display_name, "OldNick"); // Renamed from nickname
    assert_eq!(migrated.email, ""); // Default value

    Ok(())
}

#[test]
fn test_hifumi_v3_to_yuuka_v3() -> Result<()> {
    let hifumi_profile = VersionedProfile {
        id: 100,
        display_name: "NewUser".to_string(),
        email: "user@example.com".to_string(),
    };

    let yuuka_profile: YuukaProfileV3 = hifumi_to_yuuka(&hifumi_profile)?;

    assert_eq!(yuuka_profile.id, 100);
    assert_eq!(yuuka_profile.display_name, "NewUser");
    assert_eq!(yuuka_profile.email, "user@example.com");

    Ok(())
}

// ============================================================================
// Scenario 3: Array fields
// ============================================================================

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaTeam {
        name: String,
        members: Vec<String>,
    }
);

#[version("0.1")]
#[derive(Debug, Clone, PartialEq)]
struct HifumiTeam {
    name: String,
    members: Vec<String>,
}

#[test]
fn test_array_field_conversion() -> Result<()> {
    let yuuka_team = YuukaTeam {
        name: "Game Dev".to_string(),
        members: vec![
            "Momoi".to_string(),
            "Midori".to_string(),
            "Yuzu".to_string(),
            "Arisu".to_string(),
        ],
    };

    let hifumi_team: HifumiTeam = yuuka_to_hifumi_with_version(&yuuka_team, "0.1")?;

    assert_eq!(hifumi_team.name, "Game Dev");
    assert_eq!(hifumi_team.members.len(), 4);
    assert_eq!(hifumi_team.members[0], "Momoi");

    // Roundtrip
    let back_to_yuuka: YuukaTeam = hifumi_to_yuuka(&hifumi_team)?;
    assert_eq!(yuuka_team, back_to_yuuka);

    Ok(())
}

// ============================================================================
// Scenario 4: Optional fields
// ============================================================================

derive_struct!(
    #[derive(Serialize, Deserialize, PartialEq)]
    pub YuukaSettings {
        theme: String,
        font_size?: i32,
    }
);

#[version("0.1")]
#[derive(Debug, Clone, PartialEq)]
struct HifumiSettings {
    theme: String,
    font_size: Option<i32>,
}

#[test]
fn test_optional_field_with_value() -> Result<()> {
    let yuuka_settings = YuukaSettings {
        theme: "dark".to_string(),
        font_size: Some(14),
    };

    let hifumi_settings: HifumiSettings = yuuka_to_hifumi_with_version(&yuuka_settings, "0.1")?;

    assert_eq!(hifumi_settings.theme, "dark");
    assert_eq!(hifumi_settings.font_size, Some(14));

    Ok(())
}

#[test]
fn test_optional_field_without_value() -> Result<()> {
    let yuuka_settings = YuukaSettings {
        theme: "light".to_string(),
        font_size: None,
    };

    let hifumi_settings: HifumiSettings = yuuka_to_hifumi_with_version(&yuuka_settings, "0.1")?;

    assert_eq!(hifumi_settings.theme, "light");
    assert_eq!(hifumi_settings.font_size, None);

    Ok(())
}
