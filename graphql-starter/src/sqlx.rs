//! Utilities to work with [sqlx]

/// Similar to `sqlx::query_as!` but with pagination capabilities.
/// 
/// **Note**: this macro won't populate `total_items` in the resulting page, it must be queried afterwards if needed.
#[macro_export]
macro_rules! sqlx_query_paginated_as {
    ($page:ident, $executor:expr, [$($cols:ident $(. $order:ident())? : $ty:path),*], $out_struct:path, $query:expr) => (
        $crate::sqlx_query_paginated_as!(
            columns = [$($cols $(. $order())? : $ty),*],
            page = $page,
            executor = $executor,
            record = $out_struct,
            query = $query,
            args = []
        )
    );

    ($page:ident, $executor:expr, [$($cols:ident $(. $order:ident())? : $ty:path),*], $out_struct:path, $query:expr, $($args:tt)*) => (
        $crate::sqlx_query_paginated_as!(
            columns = [$($cols $(. $order())? : $ty),*],
            page = $page,
            executor = $executor,
            record = $out_struct,
            query = $query,
            args = [$($args)*]
        )
    );

    (
        columns = [$($cols:ident $(. $order:ident())? : $ty:path),*],
        page = $page:ident,
        executor = $executor:expr,
        record = $out_struct:path,
        query = $query:expr,
        args = [$($args:expr),*]
    ) => ({
        use $crate::{
            error::{GenericErrorCode, MapToErr},
            pagination::{IntoCursorVec, Page, PageQuery},
        };
        let limit;
        let backward;
        let mut rows = match $page {
            PageQuery::Forward(page) => {
                backward = false;
                limit = page.first;
                if let Some(after) = page.after {
                    let after: ($($ty,)*) = after.as_data()?;
                    tracing::trace!("Fetching data after: {after:#?}");
                    $crate::sqlx_expand_paginated_query!(
                        record = $out_struct,
                        query = $query,
                        args = [$($args),*],
                        extra_row = true,
                        columns = [$($cols $(. $order())?),*],
                        first = (page.first as i64),
                        after = after
                    )
                    .fetch_all($executor)
                    .await
                } else {
                    $crate::sqlx_expand_paginated_query!(
                        record = $out_struct,
                        query = $query,
                        args = [$($args),*],
                        extra_row = true,
                        columns = [$($cols $(. $order())?),*],
                        first = (page.first as i64)
                    )
                    .fetch_all($executor)
                    .await
                }
            }
            PageQuery::Backward(page) => {
                backward = true;
                limit = page.last;
                if let Some(before) = page.before {
                    let before: ($($ty,)*) = before.as_data()?;
                    tracing::trace!("Fetching data before: {before:#?}");
                    $crate::sqlx_expand_paginated_query!(
                        record = $out_struct,
                        query = $query,
                        args = [$($args),*],
                        extra_row = true,
                        columns = [$($cols $(. $order())?),*],
                        last = (page.last as i64),
                        before = before
                    )
                    .fetch_all($executor)
                    .await
                } else {
                    $crate::sqlx_expand_paginated_query!(
                        record = $out_struct,
                        query = $query,
                        args = [$($args),*],
                        extra_row = true,
                        columns = [$($cols $(. $order())?),*],
                        last = (page.last as i64)
                    )
                    .fetch_all($executor)
                    .await
                }
            }
        }
        .map_to_err(
            GenericErrorCode::InternalServerError,
            "Error fetching paginated query",
        )?;

        let mut has_previous_page = false;
        let mut has_next_page = false;
        if rows.len() > limit {
            if backward {
                has_previous_page = true;
                rows.remove(0);
            } else {
                has_next_page = true;
                rows.remove(rows.len() - 1);
            }
        }

        Page::from_iter(
            has_previous_page,
            has_next_page,
            None,
            rows.with_cursor(|r| $crate::struct_to_tuple!(r => $($cols),*))?,
        )
    });
}

#[macro_export]
/// Builds a tuple from the given struct fields
macro_rules! struct_to_tuple {
    ($struct:ident => $($field:ident),*) => {
        ( $($struct . $field),* )
    };
}
