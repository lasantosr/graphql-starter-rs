use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse_quote, Expr, Ident};

pub mod input;

pub(crate) fn r#impl(input: input::QueryInput) -> TokenStream {
    // Retrieve common input
    let record = input.record;
    let query = input.query;
    let mut args = input.arg_exprs;

    // Match page info
    match (input.first, input.last, input.after, input.before) {
        // If there's no page set, just expand the query
        (None, None, None, None) => {
            if args.is_empty() {
                quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query))
            } else {
                quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query, args = [#( #args ),*]))
            }
        }
        // When only `first` is set
        (Some(first), None, None, None) => {
            if input.extra_row {
                args.push(parse_quote!((#first) + 1i64));
            } else {
                args.push(first);
            }
            let limit = args.len();
            let order = columns_to_str(&input.columns, false);
            let query = format!("{query} ORDER BY {order} LIMIT ${limit}");
            quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query, args = [#( #args ),*]))
        }
        // When both `first` and `after` is set
        (Some(first), None, Some(after), None) => {
            if input.extra_row {
                args.push(parse_quote!((#first) + 1i64));
            } else {
                args.push(first);
            }
            let limit = args.len();
            let order = columns_to_str(&input.columns, false);
            let filter = columns_to_filter(&input.columns, after, true, &mut args);
            let query = format!("SELECT * FROM ({query}) as q WHERE {filter} ORDER BY {order} LIMIT ${limit}");
            quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query, args = [#( #args ),*]))
        }
        // When only `last` is set
        (None, Some(last), None, None) => {
            if input.extra_row {
                args.push(parse_quote!((#last) + 1i64));
            } else {
                args.push(last);
            }
            let limit = args.len();
            let order = columns_to_str(&input.columns, false);
            let order_reverse = columns_to_str(&input.columns, true);
            let query =
                format!("SELECT * FROM ({query} ORDER BY {order_reverse} LIMIT ${limit}) as q ORDER BY {order}");
            quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query, args = [#( #args ),*]))
        }
        // When both `last` and `before` is set
        (None, Some(last), None, Some(before)) => {
            if input.extra_row {
                args.push(parse_quote!((#last) + 1i64));
            } else {
                args.push(last);
            }
            let limit = args.len();
            let order = columns_to_str(&input.columns, false);
            let order_reverse = columns_to_str(&input.columns, true);
            let filter = columns_to_filter(&input.columns, before, false, &mut args);
            let query = format!(
                "SELECT * FROM (SELECT * FROM ({query}) as q WHERE {filter} ORDER BY {order_reverse} LIMIT ${limit}) \
                 as o ORDER BY {order}"
            );
            quote!(sqlx::sqlx_macros::expand_query!(record = #record, source = #query, args = [#( #args ),*]))
        }
        // Any other combination
        _ => syn::Error::new(
            Span::call_site(),
            "Internal error while processing the macro: unexpected page info",
        )
        .to_compile_error(),
    }
}

/// Joins the column names with their specified order or reversed
fn columns_to_str(columns: &[(Ident, bool)], reverse: bool) -> String {
    columns
        .iter()
        .map(|(column_name, asc)| {
            let order_asc = if reverse { !*asc } else { *asc };
            format!(r#""{column_name}" {}"#, if order_asc { "ASC" } else { "DESC" })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Produces a where filter for the given columns considering its order and `after` flag
///
/// The values will be appended to `args`
fn columns_to_filter(columns: &[(Ident, bool)], values: Expr, after: bool, args: &mut Vec<Expr>) -> String {
    let mut filters = Vec::new();

    let mut prev: Option<String> = None;
    for (ix, (column_name, column_asc)) in columns.iter().enumerate() {
        // Push the column filter argument to the argument list
        let ix = proc_macro2::Literal::usize_unsuffixed(ix);
        args.push(parse_quote!(#values . #ix));
        let val_ref = args.len();

        // Calculate the filter just for the current column based on its ordering
        let current_filter = if (*column_asc && after) || (!*column_asc && !after) {
            format!(r#""{column_name}" > ${val_ref}"#)
        } else {
            format!(r#""{column_name}" < ${val_ref}"#)
        };

        // Calculate the whole filter based on the previous columns and update `prev`
        let filter: String;
        if let Some(prev) = &mut prev {
            filter = format!(r#"({prev} AND {current_filter})"#);
            *prev = format!(r#"{prev} AND "{column_name}" = ${val_ref}"#);
        } else {
            filter = current_filter;
            prev = Some(format!(r#""{column_name}" = ${val_ref}"#));
        }

        // Push the filter
        filters.push(filter);
    }

    filters.join(" OR ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_simple() {
        let input = quote!(
            record = MyRow,
            query = r#"SELECT "id", "name" FROM rows"#,
            columns = [id.desc()],
            first = 10i64
        );
        let input = syn::parse2::<input::QueryInput>(input).unwrap();

        let output = r#impl(input);
        #[rustfmt::skip]
        let expected = quote!(
            sqlx::sqlx_macros::expand_query!(
                record = MyRow, 
                source = "SELECT \"id\", \"name\" FROM rows ORDER BY \"id\" DESC LIMIT $1",
                args = [10i64]
            )
        );

        assert_eq!(expected.to_string(), output.to_string());
    }

    #[test]
    fn test_first_extra() {
        let input = quote!(
            record = MyRow,
            query = r#"SELECT "id", "name" FROM rows WHERE tenant = $1"#,
            args = [tenant],
            columns = [id],
            extra_row = true,
            first = 10i64
        );
        let input = syn::parse2::<input::QueryInput>(input).unwrap();

        let output = r#impl(input);
        #[rustfmt::skip]
        let expected = quote!(
            sqlx::sqlx_macros::expand_query!(
                record = MyRow, 
                source = "SELECT \"id\", \"name\" FROM rows WHERE tenant = $1 ORDER BY \"id\" ASC LIMIT $2",
                args = [tenant, (10i64) + 1i64]
            )
        );

        assert_eq!(expected.to_string(), output.to_string());
    }

    #[test]
    fn test_first_after_multiple() {
        let input = quote!(
            record = MyRow,
            query = r#"SELECT "id", "name" FROM rows WHERE tenant = $1"#,
            args = [tenant],
            columns = [name.asc(), id.desc()],
            extra_row = true,
            first = 10i64,
            after = after
        );
        let input = syn::parse2::<input::QueryInput>(input).unwrap();

        let output = r#impl(input);
        #[rustfmt::skip]
        let expected = quote!(
            sqlx::sqlx_macros::expand_query!(
                record = MyRow, 
                source = "SELECT * FROM (SELECT \"id\", \"name\" FROM rows WHERE tenant = $1) as q WHERE \"name\" > $3 OR (\"name\" = $3 AND \"id\" < $4) ORDER BY \"name\" ASC, \"id\" DESC LIMIT $2", 
                args = [tenant, (10i64) + 1i64, after.0, after.1]
            )
        );

        assert_eq!(expected.to_string(), output.to_string());
    }
}
