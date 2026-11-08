use proc_macro2::TokenStream;
use syn::{
    braced, parenthesized,
    parse::{Parse, ParseStream},
    token, Ident, Token, TypePath,
};

#[derive(Clone)]
pub enum MigrationField {
    Add {
        value: (Ident, TypePath),
        converter: Option<TokenStream>,
    },
    Remove {
        value: (Ident, TypePath),
        /// 反向（降级）重建该字段的表达式；缺省则该步不可逆。
        back: Option<TokenStream>,
    },
    Rename {
        source: Vec<(Ident, TypePath)>,
        target: (Ident, TypePath),
        converter: Option<TokenStream>,
        /// 反向（降级）重建源字段的表达式；单源直接求值为源字段类型，
        /// 多源求值为元组。缺省时仅「单源、无变换、同类型」的纯改名可逆。
        back: Option<TokenStream>,
    },
    Copy {
        source: Vec<(Ident, TypePath)>,
        target: (Ident, TypePath),
        converter: Option<TokenStream>,
    },
}

/// 解析可选的反向转换括号：`{ back: <expr> }`（标签必写，防与前向表达式歧义）。
fn parse_back_expr(input: ParseStream) -> syn::Result<Option<TokenStream>> {
    if !input.peek(token::Brace) {
        return Ok(None);
    }

    let content;
    braced!(content in input);
    parse_back_body(&content)
}

/// peek 判定「下一个是 `标签:` 而非路径开头」：`ident` 后跟**单**冒号。
/// syn 对单字符标点的 peek 不查 jointness，`Token![:]` 会匹配 `::` 的首个
/// 冒号，因此必须显式排除 `Token![::]`（多字符标点的 peek 要求联合，
/// 恰好只在真路径分隔符时为真）。
fn peek_back_label(content: ParseStream) -> bool {
    content.peek(Ident) && content.peek2(Token![:]) && !content.peek2(Token![::])
}

fn parse_back_body(content: ParseStream) -> syn::Result<Option<TokenStream>> {
    if peek_back_label(content) {
        let label = content.parse::<Ident>()?;
        if label == "back" {
            content.parse::<Token![:]>()?;
            let expr = content.parse::<TokenStream>()?;
            if !content.is_empty() {
                return Err(content.error("unexpected tokens after the backward converter"));
            }
            return Ok(Some(expr));
        }
    }
    Err(content.error("expected a backward converter brace `{ back: <expr> }`"))
}

/// 解析 rename 的第一个括号：普通前向表达式，或 `back:` 标签的反向表达式。
fn parse_converter_or_back(
    input: ParseStream,
) -> syn::Result<(Option<TokenStream>, Option<TokenStream>)> {
    if !input.peek(token::Brace) {
        return Ok((None, None));
    }

    let content;
    braced!(content in input);

    if peek_back_label(&content) {
        // 到这里只可能是 `back:` 标签：合法表达式不会以 `ident :`（单冒号）
        // 开头——路径开头已被 peek_back_label 排除。
        if let Some(back) = parse_back_body(&content)? {
            return Ok((None, Some(back)));
        }
        return Err(content.error("expected a forward converter expression"));
    }

    let converter = content.parse::<TokenStream>()?;
    Ok((Some(converter), None))
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

            // - a: ty { back: <expr> },
            let key = input.parse::<Ident>()?;
            input.parse::<Token![:]>()?;
            let ty = input.parse::<TypePath>()?;
            let back = parse_back_expr(input)?;

            Ok(Self::Remove {
                value: (key, ty),
                back,
            })
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
            if peek_back_label(&content) {
                return Err(content.error(
                    "multi-source rename requires a forward converter; \
                     put the backward converter in a second brace: { ... } { back: <expr> }",
                ));
            }
            let converter = content.parse::<TokenStream>()?;
            let back = parse_back_expr(input)?;

            Ok(Self::Rename {
                source,
                target: (target_ident, target_ty),
                converter: Some(converter),
                back,
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

                    // a: ty => b: ty { <fwd> } { back: <expr> },
                    let (converter, first_back) = parse_converter_or_back(input)?;
                    let back = match first_back {
                        Some(back) => Some(back),
                        None => parse_back_expr(input)?,
                    };

                    Ok(Self::Rename {
                        source: vec![(source_key, source_ty)],
                        target: (target_key, target_ty),
                        converter,
                        back,
                    })
                } else {
                    let target_ty = input.parse::<TypePath>()?;

                    // a: ty => ty { <fwd> } { back: <expr> },
                    let (converter, first_back) = parse_converter_or_back(input)?;
                    let back = match first_back {
                        Some(back) => Some(back),
                        None => parse_back_expr(input)?,
                    };

                    Ok(Self::Rename {
                        source: vec![(source_key.clone(), source_ty.clone())],
                        target: (source_key, target_ty),
                        converter,
                        back,
                    })
                }
            } else {
                input.parse::<Token![=>]>()?;
                let target_key = input.parse::<Ident>()?;
                input.parse::<Token![:]>()?;
                let ty = input.parse::<TypePath>()?;

                // a => b: ty { <fwd> } { back: <expr> },
                let (converter, first_back) = parse_converter_or_back(input)?;
                let back = match first_back {
                    Some(back) => Some(back),
                    None => parse_back_expr(input)?,
                };

                Ok(Self::Rename {
                    source: vec![(source_key, ty.clone())],
                    target: (target_key, ty),
                    converter,
                    back,
                })
            }
        }
    }
}
