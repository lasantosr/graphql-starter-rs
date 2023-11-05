use async_graphql::{Context, SelectionField};

use crate::queried_fields::QueriedFields;

/// Trait to convert to a [QueriedFields]
pub trait ContextQueriedFields {
    /// Extracts the [QueriedFields] from the given context, skipping the current top-level field.
    ///
    /// ## Examples
    ///
    /// Given the following query:
    ///
    /// ```graphql
    /// query {
    ///   foo {
    ///     a
    ///     b
    ///     bar {
    ///       c
    ///       d
    ///     }
    ///   }
    /// }
    /// ```
    ///
    /// The [QueriedFields] would contain different fields depending where it's called:
    ///
    /// - In the `foo` resolver, `["a", "b", "bar.c", "bar.d"]`
    /// - In the `bar` resolver within `foo`, `["c", "d"]`
    fn queried_fields(&self) -> QueriedFields;
}

impl ContextQueriedFields for &Context<'_> {
    fn queried_fields(&self) -> QueriedFields {
        let ctx = self.look_ahead();
        let mut fields = Vec::new();
        // There's always just one, the top field being queried, but we iter just in case future updates include more
        for top_field in ctx.selection_fields() {
            // Iterate first-level fields and extract all of their inner fields
            for field in top_field.selection_set() {
                push_fiend_and_extract_inner(field, None, &mut fields)
            }
        }
        QueriedFields::Fields(fields)
    }
}

/// Pushes the given [SelectionField] to the vec and extracts the inner queried fields recursively.
///
/// The fields will be nested using dots (`.`)
fn push_fiend_and_extract_inner(field: SelectionField, parent: Option<&str>, fields: &mut Vec<String>) {
    // Build the full qualified name for the field, including the parent
    let full_name = match parent {
        Some(parent) => format!("{parent}.{}", field.name()),
        None => field.name().to_string(),
    };
    // Recursively push inner fields
    for inner_field in field.selection_set() {
        push_fiend_and_extract_inner(inner_field, Some(&full_name), fields);
    }
    // Push the field
    fields.push(full_name);
}
