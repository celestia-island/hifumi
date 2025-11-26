use anyhow::{anyhow, Context, Result};
use colored::Colorize;
use git2::Repository;
use std::collections::BTreeMap;
use syn::{Fields, Item, ItemStruct, Type};

/// Represents a field change between two versions
#[derive(Debug, Clone)]
pub enum FieldChange {
    /// Field was added
    Added {
        name: String,
        ty: String,
    },
    /// Field was removed
    Removed {
        name: String,
        ty: String,
    },
    /// Field type was changed
    TypeChanged {
        name: String,
        old_ty: String,
        new_ty: String,
    },
    /// Field was renamed (detected by matching types)
    Renamed {
        old_name: String,
        new_name: String,
        ty: String,
    },
}

impl std::fmt::Display for FieldChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldChange::Added { name, ty } => {
                write!(f, "{} {}: {}", "+".green(), name.green(), ty)
            }
            FieldChange::Removed { name, ty } => {
                write!(f, "{} {}: {}", "-".red(), name.red(), ty)
            }
            FieldChange::TypeChanged { name, old_ty, new_ty } => {
                write!(
                    f,
                    "{} {}: {} => {}",
                    "~".yellow(),
                    name.yellow(),
                    old_ty.red(),
                    new_ty.green()
                )
            }
            FieldChange::Renamed { old_name, new_name, ty } => {
                write!(
                    f,
                    "{} {} => {}: {}",
                    "→".blue(),
                    old_name.red(),
                    new_name.green(),
                    ty
                )
            }
        }
    }
}

/// Result of struct analysis
pub struct AnalysisResult {
    pub struct_name: String,
    pub changes: Vec<FieldChange>,
}

impl std::fmt::Display for AnalysisResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Struct: {}", self.struct_name.bold())?;
        writeln!(f, "Changes:")?;
        for change in &self.changes {
            writeln!(f, "  {}", change)?;
        }
        Ok(())
    }
}

/// Get file content at a specific commit
fn get_file_at_commit(repo: &Repository, file_path: &str, commit_ref: &str) -> Result<String> {
    let obj = repo
        .revparse_single(commit_ref)
        .with_context(|| format!("Failed to parse commit ref: {}", commit_ref))?;

    let commit = obj
        .peel_to_commit()
        .with_context(|| format!("Failed to get commit: {}", commit_ref))?;

    let tree = commit.tree()?;

    let entry = tree
        .get_path(std::path::Path::new(file_path))
        .with_context(|| format!("File not found in commit {}: {}", commit_ref, file_path))?;

    let blob = entry
        .to_object(repo)?
        .peel_to_blob()
        .with_context(|| "Failed to get blob")?;

    let content = std::str::from_utf8(blob.content())
        .with_context(|| "File content is not valid UTF-8")?;

    Ok(content.to_string())
}

/// Parse a Rust file and extract struct fields
fn parse_struct_fields(content: &str, struct_name: &str) -> Result<BTreeMap<String, String>> {
    let file = syn::parse_file(content).with_context(|| "Failed to parse Rust file")?;

    for item in file.items {
        if let Item::Struct(ItemStruct {
            ident,
            fields: Fields::Named(named),
            ..
        }) = item
        {
            if ident == struct_name {
                let fields = named
                    .named
                    .iter()
                    .filter_map(|f| {
                        let name = f.ident.as_ref()?.to_string();
                        let ty = type_to_string(&f.ty);
                        Some((name, ty))
                    })
                    .collect();
                return Ok(fields);
            }
        }
    }

    Err(anyhow!("Struct '{}' not found in the file", struct_name))
}

/// Convert a syn::Type to a string representation
fn type_to_string(ty: &Type) -> String {
    quote::quote!(#ty).to_string().replace(" ", "")
}

/// Analyze struct changes between two commits
pub fn analyze_struct_changes(
    file_path: &str,
    struct_name: &str,
    from_commit: &str,
    to_commit: &str,
) -> Result<AnalysisResult> {
    let repo = Repository::discover(".").with_context(|| "Failed to find git repository")?;

    let old_content = get_file_at_commit(&repo, file_path, from_commit)?;
    let new_content = get_file_at_commit(&repo, file_path, to_commit)?;

    let old_fields = parse_struct_fields(&old_content, struct_name)?;
    let new_fields = parse_struct_fields(&new_content, struct_name)?;

    let changes = detect_changes(&old_fields, &new_fields);

    Ok(AnalysisResult {
        struct_name: struct_name.to_string(),
        changes,
    })
}

/// Detect changes between two field maps
fn detect_changes(
    old_fields: &BTreeMap<String, String>,
    new_fields: &BTreeMap<String, String>,
) -> Vec<FieldChange> {
    let mut changes = Vec::new();
    let mut matched_old: std::collections::HashSet<&String> = std::collections::HashSet::new();
    let mut matched_new: std::collections::HashSet<&String> = std::collections::HashSet::new();

    // Find fields that exist in both with same name
    for (name, old_ty) in old_fields {
        if let Some(new_ty) = new_fields.get(name) {
            if old_ty != new_ty {
                changes.push(FieldChange::TypeChanged {
                    name: name.clone(),
                    old_ty: old_ty.clone(),
                    new_ty: new_ty.clone(),
                });
            }
            matched_old.insert(name);
            matched_new.insert(name);
        }
    }

    // Find potential renames (same type, different name)
    let unmatched_old: Vec<_> = old_fields
        .iter()
        .filter(|(k, _)| !matched_old.contains(k))
        .collect();
    let unmatched_new: Vec<_> = new_fields
        .iter()
        .filter(|(k, _)| !matched_new.contains(k))
        .collect();

    let mut renamed_old: std::collections::HashSet<&String> = std::collections::HashSet::new();
    let mut renamed_new: std::collections::HashSet<&String> = std::collections::HashSet::new();

    for (old_name, old_ty) in &unmatched_old {
        for (new_name, new_ty) in &unmatched_new {
            if old_ty == new_ty && !renamed_new.contains(new_name) {
                changes.push(FieldChange::Renamed {
                    old_name: (*old_name).clone(),
                    new_name: (*new_name).clone(),
                    ty: (*old_ty).clone(),
                });
                renamed_old.insert(old_name);
                renamed_new.insert(new_name);
                break;
            }
        }
    }

    // Remaining unmatched old fields are removed
    for (name, ty) in &unmatched_old {
        if !renamed_old.contains(name) {
            changes.push(FieldChange::Removed {
                name: (*name).clone(),
                ty: (*ty).clone(),
            });
        }
    }

    // Remaining unmatched new fields are added
    for (name, ty) in &unmatched_new {
        if !renamed_new.contains(name) {
            changes.push(FieldChange::Added {
                name: (*name).clone(),
                ty: (*ty).clone(),
            });
        }
    }

    changes
}
