use proc_macro2::Span;
use syn::{
    bracketed, parenthesized,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token, Expr, ExprArray, Ident, LitBool, LitStr, Result, Token, Type,
};

struct ColumnsArray {
    v: Vec<(Ident, bool)>,
}

impl Parse for ColumnsArray {
    #[inline]
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        let _ = bracketed!(content in input);
        let mut v = vec![];
        let mut expect_comma = false;

        while !content.is_empty() {
            if expect_comma {
                let _ = content.parse::<token::Comma>()?;
            }

            let column_name = content.parse::<Ident>()?;
            let mut asc = true;
            if content.peek(token::Dot) {
                let _ = content.parse::<token::Dot>()?;
                let order = content.parse::<Ident>()?;
                asc = match order.to_string().to_lowercase().as_str() {
                    "asc" => true,
                    "desc" => false,
                    _ => return Err(syn::Error::new(order.span(), "only 'asc' or 'desc' are allowed")),
                };
                let inner_content;
                let _ = parenthesized!(inner_content in content);
                if !inner_content.is_empty() {
                    return Err(syn::Error::new(order.span(), "no arguments are allowed"));
                }
            }

            v.push((column_name, asc));

            expect_comma = true;
        }
        Ok(Self { v })
    }
}

pub struct QueryInput {
    pub(super) record: Type,
    pub(super) query: String,
    pub(super) arg_exprs: Vec<Expr>,
    pub(super) extra_row: bool,
    pub(super) columns: Vec<(Ident, bool)>,
    pub(super) first: Option<Expr>,
    pub(super) last: Option<Expr>,
    pub(super) after: Option<Expr>,
    pub(super) before: Option<Expr>,
}

impl Parse for QueryInput {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut record: Option<Type> = None;
        let mut query: Option<String> = None;
        let mut arg_exprs: Option<Vec<Expr>> = None;
        let mut extra_row = false;
        let mut columns: Option<Vec<(Ident, bool)>> = None;
        let mut first: Option<Expr> = None;
        let mut last: Option<Expr> = None;
        let mut after: Option<Expr> = None;
        let mut before: Option<Expr> = None;

        let mut expect_comma = false;

        while !input.is_empty() {
            if expect_comma {
                let _ = input.parse::<token::Comma>()?;
            }

            let key: Ident = input.parse()?;
            let _ = input.parse::<syn::token::Eq>()?;

            if key == "record" {
                record = Some(input.parse()?);
            } else if key == "query" {
                let query_str = Punctuated::<LitStr, Token![+]>::parse_separated_nonempty(input)?
                    .iter()
                    .map(LitStr::value)
                    .collect();
                query = Some(query_str);
            } else if key == "args" {
                let exprs = input.parse::<ExprArray>()?;
                arg_exprs = Some(exprs.elems.into_iter().collect())
            } else if key == "extra_row" {
                let extra = input.parse::<LitBool>()?;
                extra_row = extra.value;
            } else if key == "columns" {
                let cols = input.parse::<ColumnsArray>()?;
                columns = Some(cols.v);
            } else if key == "first" {
                first = Some(input.parse()?);
            } else if key == "last" {
                last = Some(input.parse()?);
            } else if key == "after" {
                after = Some(input.parse()?);
            } else if key == "before" {
                before = Some(input.parse()?);
            } else {
                let message = format!("unexpected input key: {key}");
                return Err(syn::Error::new_spanned(key, message));
            }

            expect_comma = true;
        }

        let ret = QueryInput {
            record: record.ok_or_else(|| input.error("expected `record` key"))?,
            query: query.ok_or_else(|| input.error("expected `query` key"))?,
            arg_exprs: arg_exprs.unwrap_or_default(),
            extra_row,
            columns: columns.ok_or_else(|| input.error("expected `columns` key"))?,
            first,
            last,
            after,
            before,
        };

        if ret.columns.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "At least one column must be specified",
            ));
        }

        if ret.first.is_some() && ret.last.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "Only one of 'first' or 'last' can be set",
            ));
        }

        if ret.first.is_some() && ret.before.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "'before' can only be used with 'last'",
            ));
        }

        if ret.last.is_some() && ret.after.is_some() {
            return Err(syn::Error::new(
                Span::call_site(),
                "'after' can only be used with 'first'",
            ));
        }

        Ok(ret)
    }
}
