use proc_macro2::TokenStream;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    token, Ident, Token, TypePath,
};

#[derive(Debug, Clone)]
pub enum MigrationField {
    Add {
        value: (Ident, TypePath),
        converter: Option<TokenStream>,
    },
    Remove {
        value: (Ident, TypePath),
    },
    Rename {
        source: Vec<(Ident, TypePath)>,
        target: (Ident, TypePath),
        converter: Option<TokenStream>,
    },
    Copy {
        source: Vec<(Ident, TypePath)>,
        target: (Ident, TypePath),
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
                    let ty = content.parse::<TypePath>()?;

                    source.push((key, ty));

                    if content.peek(Token![,]) {
                        content.parse::<Token![,]>()?;
                    }
                }

                input.parse::<Token![=>]>()?;

                let target_ident = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let ty = input.parse::<TypePath>()?;

                let content;
                braced!(content in input);
                let converter = content.parse::<TokenStream>()?;

                Ok(Self::Copy {
                    source,
                    target: (target_ident, ty),
                    converter: Some(converter),
                })
            } else {
                let source_ident = input.parse::<Ident>()?;
                if input.peek(Token![:]) {
                    input.parse::<Token![:]>()?;
                    let source_ty = input.parse::<TypePath>()?;

                    if input.peek(Token![=>]) {
                        input.parse::<Token![=>]>()?;

                        let target_ident = input.parse::<Ident>()?;
                        input.parse::<Token![:]>()?;
                        let target_ty = input.parse::<TypePath>()?;

                        if input.peek(token::Brace) {
                            // + a: ty => b: ty { ... },
                            let content;
                            braced!(content in input);
                            let converter = content.parse::<TokenStream>()?;

                            Ok(Self::Copy {
                                source: vec![(source_ident, source_ty)],
                                target: (target_ident, target_ty),
                                converter: Some(converter),
                            })
                        } else {
                            // + a: ty => b: ty,
                            Ok(Self::Copy {
                                source: vec![(source_ident, source_ty)],
                                target: (target_ident, target_ty),
                                converter: None,
                            })
                        }
                    } else if input.peek(token::Brace) {
                        // + a: ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::Add {
                            value: (source_ident, source_ty),
                            converter: Some(converter),
                        })
                    } else {
                        // + a: ty,
                        Ok(Self::Add {
                            value: (source_ident, source_ty),
                            converter: None,
                        })
                    }
                } else {
                    input.parse::<Token![=>]>()?;
                    let target_ident = input.parse::<Ident>()?;
                    input.parse::<Token![:]>()?;
                    let ty = input.parse::<TypePath>()?;

                    if input.peek(token::Brace) {
                        // + a => b: ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::Copy {
                            source: vec![(source_ident, ty.clone())],
                            target: (target_ident, ty),
                            converter: Some(converter),
                        })
                    } else {
                        // + a => b: ty,
                        Ok(Self::Copy {
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
            let ty = input.parse::<TypePath>()?;

            Ok(Self::Remove { value: (key, ty) })
        } else if input.peek(token::Paren) {
            // (a: ty, b: ty, ...) => c: ty { ... },
            let content;
            parenthesized!(content in input);

            let mut source = vec![];
            while !content.is_empty() {
                let key = content.parse::<Ident>()?;
                content.parse::<Token![:]>()?;
                let ty = content.parse::<TypePath>()?;

                source.push((key, ty));

                if content.peek(Token![,]) {
                    content.parse::<Token![,]>()?;
                }
            }

            input.parse::<Token![=>]>()?;

            let target_ident = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            let target_ty = input.parse::<TypePath>()?;

            let content;
            braced!(content in input);
            let converter = content.parse::<TokenStream>()?;

            Ok(Self::Rename {
                source,
                target: (target_ident, target_ty),
                converter: Some(converter),
            })
        } else {
            let source_key = input.parse::<Ident>()?;

            if input.peek(Token![:]) {
                input.parse::<Token![:]>()?;
                let source_ty = input.parse::<TypePath>()?;

                input.parse::<Token![=>]>()?;

                if input.peek2(Token![:]) {
                    let target_key = input.parse::<Ident>()?;
                    input.parse::<Token![:]>()?;
                    let target_ty = input.parse::<TypePath>()?;

                    if input.peek(token::Brace) {
                        // a: ty => b: ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::Rename {
                            source: vec![(source_key, source_ty)],
                            target: (target_key, target_ty),
                            converter: Some(converter),
                        })
                    } else {
                        // a: ty => b: ty,
                        Ok(Self::Rename {
                            source: vec![(source_key, source_ty)],
                            target: (target_key, target_ty),
                            converter: None,
                        })
                    }
                } else {
                    let target_ty = input.parse::<TypePath>()?;
                    if input.peek(token::Brace) {
                        // a: ty => ty { ... },
                        let content;
                        braced!(content in input);
                        let converter = content.parse::<TokenStream>()?;

                        Ok(Self::Rename {
                            source: vec![(source_key.clone(), source_ty.clone())],
                            target: (source_key, target_ty),
                            converter: Some(converter),
                        })
                    } else {
                        // a: ty => ty,
                        Ok(Self::Rename {
                            source: vec![(source_key.clone(), source_ty.clone())],
                            target: (source_key, target_ty),
                            converter: None,
                        })
                    }
                }
            } else {
                input.parse::<Token![=>]>()?;
                let target_key = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let ty = input.parse::<TypePath>()?;
                if input.peek(token::Brace) {
                    // a => b: ty { ... },

                    let content;
                    braced!(content in input);
                    let converter = content.parse::<TokenStream>()?;

                    Ok(Self::Rename {
                        source: vec![(source_key, ty.clone())],
                        target: (target_key, ty),
                        converter: Some(converter),
                    })
                } else {
                    // a => b: ty,

                    Ok(Self::Rename {
                        source: vec![(source_key, ty.clone())],
                        target: (target_key, ty),
                        converter: None,
                    })
                }
            }
        }
    }
}
