use proc_macro2::TokenStream;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    token, Ident, Path, Token,
};

#[derive(Debug, Clone)]
pub enum MigrationField {
    AddField {
        value: (Ident, Path),
        converter: Option<TokenStream>,
    },
    RemoveField {
        value: (Ident, Path),
    },
    RenameField {
        source: Vec<(Ident, Path)>,
        target: (Ident, Path),
        converter: Option<TokenStream>,
    },
    CopyField {
        source: Vec<(Ident, Path)>,
        target: (Ident, Path),
        converter: Option<TokenStream>,
    },
}

impl Parse for MigrationField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![+]) {
            input.parse::<Token![+]>()?;

            if input.peek(token::Paren) {
                // + (a: ty, b: ty, ...) => c: ty { ... },
                let content;
                parenthesized!(content in input);

                let mut source = vec![];
                while !content.is_empty() {
                    let key = content.parse::<Ident>()?;
                    content.parse::<Token![:]>()?;
                    let ty = content.parse::<Path>()?;

                    source.push((key, ty));

                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                    }
                }

                input.parse::<Token![=>]>()?;

                let target_ident = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let ty = input.parse::<Path>()?;

                let content;
                braced!(content in input);
                let converter = content.parse::<TokenStream>()?;

                Ok(Self::CopyField {
                    source,
                    target: (target_ident, ty),
                    converter: Some(converter),
                })
            } else {
                let source_ident = input.parse::<Ident>()?;
                if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    let source_ty = input.parse::<Path>()?;

                    if input.peek(Token![=>]) {
                        input.parse::<Token![=>]>()?;

                        let target_ident = input.parse::<Ident>()?;
                        input.parse::<Token![:]>()?;
                        let target_ty = input.parse::<Path>()?;

                        if input.peek(token::Brace) {
                            // + a: ty => b: ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::CopyField {
                                source: vec![(source_ident, source_ty)],
                                target: (target_ident, target_ty),
                                converter: Some(converter),
                            })
                        } else {
                            // + a: ty => b: ty,
                            Ok(Self::CopyField {
                                source: vec![(source_ident, source_ty)],
                                target: (target_ident, target_ty),
                                converter: None,
                            })
                        }
                    } else {
                        if input.peek(token::Brace) {
                            // + a: ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::AddField {
                                value: (source_ident, source_ty),
                                converter: Some(converter),
                            })
                        } else {
                            // + a: ty,
                            Ok(Self::AddField {
                                value: (source_ident, source_ty),
                                converter: None,
                            })
                        }
                    }
                } else {
                    input.parse::<Token![=>]>()?;
                    let target_ident = input.parse::<Ident>()?;
                    input.parse::<Token![:]>()?;
                    let ty = input.parse::<Path>()?;

                    if input.peek(token::Brace) {
                        // + a => b: ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::CopyField {
                            source: vec![(source_ident, ty.clone())],
                            target: (target_ident, ty),
                            converter: Some(converter),
                        })
                    } else {
                        // + a => b: ty,
                        Ok(Self::CopyField {
                            source: vec![(source_ident, ty.clone())],
                            target: (target_ident, ty),
                            converter: None,
                        })
                    }
                }
            }
        } else if input.peek(Token![-]) {
            input.parse::<Token![-]>()?;

            // - a: ty,
            let key = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            let ty = input.parse::<Path>()?;

            Ok(Self::RemoveField { value: (key, ty) })
        } else {
            if input.peek(token::Paren) {
                // (a: ty, b: ty, ...) => c: ty { ... },
                let content;
                parenthesized!(content in input);

                let mut source = vec![];
                while !content.is_empty() {
                    let key = content.parse::<Ident>()?;
                    content.parse::<Token![:]>()?;
                    let ty = content.parse::<Path>()?;

                    source.push((key, ty));

                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                    }
                }

                input.parse::<Token![=>]>()?;

                let target_ident = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let target_ty = input.parse::<Path>()?;

                let content;
                braced!(content in input);
                let converter = content.parse::<TokenStream>()?;

                Ok(Self::RenameField {
                    source,
                    target: (target_ident, target_ty),
                    converter: Some(converter),
                })
            } else {
                let source_key = input.parse::<Ident>()?;

                if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    let source_ty = input.parse::<Path>()?;

                    input.parse::<Token![=>]>()?;

                    let target_key = input.parse::<Ident>()?;
                    if input.peek(Token![:]) {
                        input.parse::<Token![:]>()?;
                        let target_ty = input.parse::<Path>()?;

                        if input.peek(token::Brace) {
                            // a: ty => b: ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::RenameField {
                                source: vec![(source_key, source_ty)],
                                target: (target_key, target_ty),
                                converter: Some(converter),
                            })
                        } else {
                            // a: ty => b: ty,
                            Ok(Self::RenameField {
                                source: vec![(source_key, source_ty)],
                                target: (target_key, target_ty),
                                converter: None,
                            })
                        }
                    } else {
                        if input.peek(token::Brace) {
                            // a: ty => ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::RenameField {
                                source: vec![(source_key.clone(), source_ty.clone())],
                                target: (source_key, source_ty),
                                converter: Some(converter),
                            })
                        } else {
                            // a: ty => ty,
                            Ok(Self::RenameField {
                                source: vec![(source_key.clone(), source_ty.clone())],
                                target: (source_key, source_ty),
                                converter: None,
                            })
                        }
                    }
                } else {
                    input.parse::<Token![=>]>()?;
                    let target_key = input.parse::<Ident>()?;
                    input.parse::<Token![:]>()?;
                    let ty = input.parse::<Path>()?;

                    if input.peek(token::Brace) {
                        // a => b: ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::RenameField {
                            source: vec![(source_key, ty.clone())],
                            target: (target_key, ty),
                            converter: Some(converter),
                        })
                    } else {
                        if input.peek(token::Brace) {
                            // a => b: ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::RenameField {
                                source: vec![(source_key, ty.clone())],
                                target: (target_key, ty),
                                converter: Some(converter),
                            })
                        } else {
                            // a => b: ty,
                            Ok(Self::RenameField {
                                source: vec![(source_key, ty.clone())],
                                target: (target_key, ty),
                                converter: None,
                            })
                        }
                    }
                }
            }
        }
    }
}
