use std::ops::Range;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use strum::{EnumIs, EnumTryAs};

use super::{OpaqueCursor, PaginationErrorCode};
use crate::error::{err, Error, MapToErr, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardPageQuery {
    /// How many items to return
    pub first: usize,
    /// Return items only after the given cursor (excluded)
    pub after: Option<OpaqueCursor>,
}
impl ForwardPageQuery {
    /// Deserializes and retrieves the `after` field
    pub fn deserialize_after<T>(&self) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        self.after.as_ref().map(OpaqueCursor::as_data).transpose()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackwardPageQuery {
    /// How many items to return
    pub last: usize,
    /// Return items only before the given cursor (excluded)
    pub before: Option<OpaqueCursor>,
}
impl BackwardPageQuery {
    /// Deserializes and retrieves the `before` field
    pub fn deserialize_before<T>(&self) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        self.before.as_ref().map(OpaqueCursor::as_data).transpose()
    }
}

/// Page information when querying for resources
#[derive(Debug, Clone, PartialEq, Eq, EnumTryAs, EnumIs)]
pub enum PageQuery {
    Forward(ForwardPageQuery),
    Backward(BackwardPageQuery),
}

impl PageQuery {
    /// Creates a new [PageQuery], validating the input
    pub fn new(
        mut first: Option<usize>,
        mut last: Option<usize>,
        after: Option<OpaqueCursor>,
        before: Option<OpaqueCursor>,
        default_limit: Option<usize>,
        max_page_size: Option<usize>,
    ) -> Result<Self> {
        // Set defaults if set
        if let Some(default_limit) = default_limit {
            if first.is_none() && last.is_none() {
                if before.is_some() {
                    last = Some(default_limit);
                } else {
                    first = Some(default_limit);
                }
            }
        }
        // Check wether first and last values are valid
        match (&first, &last) {
            (None, None) => return Err(err!(PaginationErrorCode::PageMissing)),
            (Some(first), _) if first < &0 => {
                return Err(err!(PaginationErrorCode::PageNegativeInput { field: "first" }));
            }
            (_, Some(last)) if last < &0 => {
                return Err(err!(PaginationErrorCode::PageNegativeInput { field: "last" }));
            }
            (Some(_), Some(_)) => return Err(err!(PaginationErrorCode::PageFirstAndLast)),
            _ => (),
        }

        // Check wether after and before values are valid
        if after.is_some() && before.is_some() {
            return Err(err!(PaginationErrorCode::PageAfterAndBefore));
        }

        if let Some(first) = first {
            // Validate maximum, if set
            if let Some(max) = max_page_size {
                if first > max {
                    return Err(err!(PaginationErrorCode::PageExceedsLimit { field: "first", max }));
                }
            }
            // Forward paginating
            if before.is_some() {
                Err(err!(PaginationErrorCode::PageForwardWithBefore))
            } else {
                Ok(PageQuery::Forward(ForwardPageQuery { first, after }))
            }
        } else if let Some(last) = last {
            // Validate maximum, if set
            if let Some(max) = max_page_size {
                if last > max {
                    return Err(err!(PaginationErrorCode::PageExceedsLimit { field: "last", max }));
                }
            }
            // Backward paginating
            if after.is_some() {
                Err(err!(PaginationErrorCode::PageBackwardWithAfter))
            } else {
                Ok(PageQuery::Backward(BackwardPageQuery { last, before }))
            }
        } else {
            unreachable!()
        }
    }

    /// Decodes the given page arguments into a [PageQuery]
    pub fn decode(
        first: Option<usize>,
        last: Option<usize>,
        after: Option<String>,
        before: Option<String>,
        default_limit: Option<usize>,
        max_page_size: Option<usize>,
    ) -> Result<Self> {
        // Decode cursors
        let after = after
            .filter(|c| !c.is_empty())
            .map(OpaqueCursor::decode)
            .transpose()
            .map_to_err_with(PaginationErrorCode::PageInvalidCursor, "Could not parse 'after' cursor")?;
        let before = before
            .filter(|c| !c.is_empty())
            .map(OpaqueCursor::decode)
            .transpose()
            .map_to_err_with(
                PaginationErrorCode::PageInvalidCursor,
                "Could not parse 'before' cursor",
            )?;

        // Return
        Self::new(first, last, after, before, default_limit, max_page_size)
    }
}

/// An edge in a [Page]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge<T> {
    /// A cursor for use in pagination
    pub cursor: OpaqueCursor,
    /// The item at the end of the edge
    pub node: T,
}
impl<T> Edge<T> {
    /// Converts the node by calling the provided closure
    pub fn map<Z, F>(self, map: F) -> Edge<Z>
    where
        F: FnOnce(T) -> Z,
    {
        Edge {
            cursor: self.cursor,
            node: map(self.node),
        }
    }

    /// Tries to convert the node by calling the provided closure
    pub fn try_map<Z, F, E>(self, map: F) -> Result<Edge<Z>, E>
    where
        F: FnOnce(T) -> Result<Z, E>,
    {
        Ok(Edge {
            cursor: self.cursor,
            node: map(self.node)?,
        })
    }
}

/// Page information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfo {
    /// When backward paginating, wether there's a previous page
    pub has_previous_page: bool,
    /// When forward paginating, wether there's a next page
    pub has_next_page: bool,
    /// The cursor of the first edge, useful when backward paginating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_cursor: Option<OpaqueCursor>,
    /// The cursor of the last edge, useful when forward paginating
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_cursor: Option<OpaqueCursor>,
}

/// One page of the cursor-based pagination
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    /// The current page information
    pub page_info: PageInfo,
    /// Total number of items queried, before pagination
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_items: Option<u64>,
    /// The edges on this page
    pub edges: Vec<Edge<T>>,
}

impl<T> Page<T> {
    /// Builds a new [Page]
    pub fn new(has_previous_page: bool, has_next_page: bool, total_items: Option<u64>, edges: Vec<Edge<T>>) -> Self {
        Self {
            page_info: PageInfo {
                has_previous_page,
                has_next_page,
                start_cursor: edges.first().map(|e| e.cursor.clone()),
                end_cursor: edges.last().map(|e| e.cursor.clone()),
            },
            total_items,
            edges,
        }
    }

    /// Builds a new [Page]
    pub fn from_iter<I: IntoIterator<Item = Edge<T>>>(
        has_previous_page: bool,
        has_next_page: bool,
        total_items: Option<u64>,
        edges: I,
    ) -> Self {
        let edges = edges.into_iter().collect();
        Self::new(has_previous_page, has_next_page, total_items, edges)
    }

    /// Sets the total_items to [Some]
    pub fn with_total_items(mut self, total_items: u64) -> Self {
        self.total_items = Some(total_items);
        self
    }

    /// Takes a closure to map each element
    pub fn map<Z, F>(self, map: F) -> Page<Z>
    where
        F: FnMut(Edge<T>) -> Edge<Z>,
    {
        Page::new(
            self.page_info.has_previous_page,
            self.page_info.has_next_page,
            self.total_items,
            self.edges.into_iter().map(map).collect(),
        )
    }

    /// Takes a closure to try to map each element
    pub fn try_map<Z, F, E>(self, map: F) -> Result<Page<Z>, E>
    where
        F: FnMut(Edge<T>) -> Result<Edge<Z>, E>,
    {
        Ok(Page::new(
            self.page_info.has_previous_page,
            self.page_info.has_next_page,
            self.total_items,
            self.edges.into_iter().map(map).collect::<Result<Vec<_>, _>>()?,
        ))
    }

    /// Takes a closure to map each element node
    pub fn map_node<Z, F>(self, map: F) -> Page<Z>
    where
        F: Fn(T) -> Z,
    {
        Page::new(
            self.page_info.has_previous_page,
            self.page_info.has_next_page,
            self.total_items,
            self.edges.into_iter().map(move |e| e.map(&map)).collect(),
        )
    }

    /// Takes a closure to try to map each element node
    pub fn try_map_node<Z, F, E>(self, map: F) -> Result<Page<Z>, E>
    where
        F: Fn(T) -> Result<Z, E>,
    {
        Ok(Page::new(
            self.page_info.has_previous_page,
            self.page_info.has_next_page,
            self.total_items,
            self.edges
                .into_iter()
                .map(move |e| e.try_map(&map))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    /// Builds a new [Page] from all of the items, useful when mocking or storage doesn't support paging
    pub fn from_items(mut items: Vec<T>, page: PageQuery) -> Result<Self> {
        // Retrieve page fields
        let (first, after, last, before): (Option<usize>, Option<usize>, Option<usize>, Option<usize>) = match page {
            PageQuery::Forward(forward) => (Some(forward.first), forward.deserialize_after()?, None, None),
            PageQuery::Backward(backward) => (None, None, Some(backward.last), backward.deserialize_before()?),
        };

        let items_len = items.len();
        let total_items = Some(items_len as u64);

        // https://relay.dev/graphql/connections.htm#sec-Pagination-algorithm

        // 1. Let edges be the result of calling ApplyCursorsToEdges(allEdges, before, after).
        // 1.1. Initialize edges to be allEdges.
        let mut start = 0usize;
        let mut end = items_len;

        // 1.2. If after is set:
        if let Some(after) = after {
            if after >= items_len {
                return Ok(Self::new(false, false, total_items, Vec::new()));
            }
            // 1.2.a. Let afterEdge be the edge in edges whose cursor is equal to the after argument.
            start = after + 1;
        }
        // 1.3. If before is set
        if let Some(before) = before {
            if before == 0 {
                return Ok(Self::new(false, false, total_items, Vec::new()));
            }
            // 1.3.a. Let beforeEdge be the edge in edges whose cursor is equal to the before argument.
            end = before;
        }

        // 1.2.b. Remove all elements of edges before and including afterEdge.
        // 1.3.b. Remove all elements of edges after and including beforeEdge.
        retain_range(&mut items, start..end);

        let items_len = items.len();
        if let Some(first) = first {
            // 2. If first is set
            // 2.b. If edges has length greater than than first, slice edges to be of length first by removing edges
            // from the end of edges.
            retain_range(&mut items, 0..first.min(items_len));
            end = start + items.len();
        } else if let Some(last) = last {
            // 3. If last is set
            // 3.b. If edges has length greater than than last, slice edges to be of length last by removing edges from
            // the start of edges.
            retain_range(&mut items, items_len - last.min(items_len)..items_len);
            start = end - items.len();
        }

        // Append the cursor to each node to create the edges
        let edges = items
            .into_iter()
            .enumerate()
            .map(|(idx, item)| {
                Ok::<_, Box<Error>>(Edge {
                    cursor: OpaqueCursor::new(&(start + idx))?,
                    node: item,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // 4. Return edges
        Ok(Self::new(start > 0, end < items_len, total_items, edges))
    }
}

// Based on https://stackoverflow.com/a/65004188
fn retain_range<T>(items: &mut Vec<T>, range: Range<usize>) {
    items.truncate(range.end);
    if range.start < items.len() {
        items.drain(0..range.start);
    } else {
        items.clear();
    }
}

impl<T> IntoIterator for Page<T> {
    type IntoIter = std::vec::IntoIter<Edge<T>>;
    type Item = Edge<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.edges.into_iter()
    }
}

/// Trait to convert iterators into edges
pub trait IntoCursorVec<T> {
    /// Maps this iterator to include the opaque cursor with each item
    fn with_cursor<F, Z>(self, cursor_generator: F) -> Result<Vec<Edge<T>>>
    where
        F: Fn(&T) -> Z + 'static,
        Z: Serialize + DeserializeOwned;
}

impl<T, Y> IntoCursorVec<T> for Y
where
    Y: IntoIterator<Item = T> + 'static,
{
    fn with_cursor<F, Z>(self, cursor_generator: F) -> Result<Vec<Edge<T>>>
    where
        F: Fn(&T) -> Z + 'static,
        Z: Serialize + DeserializeOwned,
    {
        self.into_iter()
            .map(|node| {
                Ok(Edge {
                    cursor: OpaqueCursor::new(&cursor_generator(&node))?,
                    node,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    }
}

#[cfg(test)]
mod tests {
    use base64::{prelude::BASE64_URL_SAFE_NO_PAD, Engine};

    use super::*;

    #[test]
    fn test_forward_pagination() {
        let page = Page::from_items(
            (0..5).collect(),
            PageQuery::decode(
                Some(1),
                None,
                Some(BASE64_URL_SAFE_NO_PAD.encode("3")),
                None,
                None,
                None,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(page.total_items, Some(5));
        assert!(!page.page_info.has_next_page);
        assert!(page.page_info.has_previous_page);
        assert_eq!(page.into_iter().map(|e| e.node).collect::<Vec<_>>(), vec![4]);
    }

    #[test]
    fn test_backward_pagination() {
        let page = Page::from_items(
            (0..20).collect(),
            PageQuery::decode(
                None,
                Some(6),
                None,
                Some(BASE64_URL_SAFE_NO_PAD.encode("9")),
                None,
                None,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(page.total_items, Some(20));
        assert!(!page.page_info.has_next_page);
        assert!(page.page_info.has_previous_page);
        assert_eq!(
            page.into_iter().map(|e| e.node).collect::<Vec<_>>(),
            vec![3, 4, 5, 6, 7, 8]
        );
    }
}
