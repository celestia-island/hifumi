use anyhow::Result;

use crate::analyze::{analyze_struct_changes, FieldChange};

/// Generate hifumi migration code from detected changes
pub fn generate_migration_code(
    file_path: &str,
    struct_name: &str,
    from_version: &str,
    to_version: &str,
    from_commit: &str,
    to_commit: &str,
) -> Result<String> {
    let result = analyze_struct_changes(file_path, struct_name, from_commit, to_commit)?;

    if result.changes.is_empty() {
        return Ok(format!(
            "// No changes detected between {} and {}\n#[migration(\"{from_version}\" => \"{to_version}\")]",
            from_commit, to_commit
        ));
    }

    let mut migration_rules = Vec::new();

    for change in &result.changes {
        match change {
            FieldChange::Added { name, ty } => {
                migration_rules.push(format!("    + {}: {},", name, ty));
            }
            FieldChange::Removed { name, ty } => {
                migration_rules.push(format!("    - {}: {},", name, ty));
            }
            FieldChange::TypeChanged { name, old_ty, new_ty } => {
                // Type change requires a converter
                migration_rules.push(format!(
                    "    {}: {} => {} {{ /* TODO: Add converter */ }},",
                    name, old_ty, new_ty
                ));
            }
            FieldChange::Renamed { old_name, new_name, ty } => {
                migration_rules.push(format!("    {} => {}: {},", old_name, new_name, ty));
            }
        }
    }

    let rules = migration_rules.join("\n");

    Ok(format!(
        r#"#[migration("{from_version}" => "{to_version}" {{
{rules}
}})]"#
    ))
}
