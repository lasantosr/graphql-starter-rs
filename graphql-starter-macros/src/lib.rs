#![forbid(unsafe_code)]

#[cfg(feature = "sqlx")]
mod sqlx;
#[cfg(feature = "subject")]
mod subject;

use proc_macro::TokenStream;
use proc_macro_error2::proc_macro_error;

#[cfg(feature = "sqlx")]
/// Similar to `sqlx::expand_query!` but includes pagination capabilities.
///
/// Input parameters:
/// - `record = Type` _(**mandatory**)_: The type of the retrieved record
/// - `query = String` _(**mandatory**)_: The query to be executed
/// - `args = [Expr]` _(optional)_: The arguments to `query`
/// - `extra_row = bool` _(optional)_: Wether to return an extra row or not (useful to determine if there's a
///   previous/next page)
/// - `columns = [Ident]` _(**mandatory**)_: The columns to order by, each row should be uniquely identified by this
///   combination of columns.
///   - The ordering can also be specified and defaults to `asc`. For example `[timestamp.desc(),
///   id.asc()]`
/// - `first = Expr` _(optional)_: The number of rows to return for forward pagination
/// - `last = Expr` _(optional)_: The number of rows to return for backward pagination
/// - `after = Expr` _(optional)_: The variable for a tuple with the values of the cursor for the `columns` **in the
///   same order** when forward paginating
/// - `before = Expr` _(optional)_: The variable for a tuple with the values of the cursor for the `columns` **in the
///   same order** when backward paginating
#[proc_macro_error]
#[proc_macro]
pub fn sqlx_expand_paginated_query(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as sqlx::pagination::input::QueryInput);
    sqlx::pagination::r#impl(input).into()
}

#[cfg(feature = "subject")]
/// Derives the `Subject` trait.
#[proc_macro_error]
#[proc_macro_derive(Subject)]
pub fn subject(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    subject::r#impl(input).into()
}
