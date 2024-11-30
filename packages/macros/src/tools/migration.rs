use syn::{
    parse::{Parse, ParseStream},
    Path,
};

#[derive(Debug, Clone)]
pub struct Migration {
    pub from: String,
    pub to: String,
    pub changes: Vec<Change>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    AddField {
        key: String,
        ty: Path,
    },
    RemoveField {
        key: String,
        ty: Path,
    },
    RenameField {
        source: Vec<(String, Path)>,
        target: String,
        ty: Path,
    },
    CopyField {
        source: Vec<(String, Path)>,
        target: String,
        ty: Path,
    },
}

impl Parse for Migration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(Self {})
    }
}
