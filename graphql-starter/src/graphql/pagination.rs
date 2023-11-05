use async_graphql::{
    connection::{Connection, CursorType, Edge},
    OutputType, SimpleObject,
};

use crate::{
    error::Error,
    pagination::{OpaqueCursor, Page},
};

#[derive(SimpleObject)]
pub struct ConnectionFields {
    /// Total number of items in this connection
    pub total_items: Option<u64>,
}

impl CursorType for OpaqueCursor {
    type Error = Box<Error>;

    fn decode_cursor(s: &str) -> Result<Self, Self::Error> {
        OpaqueCursor::decode(s)
    }

    fn encode_cursor(&self) -> String {
        self.encode()
    }
}

/// Trait to convert into a [Connection]
pub trait IntoConnection<T>
where
    T: OutputType,
{
    /// Converts `self` to a [Connection]
    fn into_connection(self) -> Connection<OpaqueCursor, T, ConnectionFields>;
}

impl<T> IntoConnection<T> for Page<T>
where
    T: OutputType,
{
    fn into_connection(self) -> Connection<OpaqueCursor, T, ConnectionFields> {
        let mut conn = Connection::with_additional_fields(
            self.page_info.has_previous_page,
            self.page_info.has_next_page,
            ConnectionFields {
                total_items: self.total_items,
            },
        );
        conn.edges
            .extend(self.edges.into_iter().map(|e| Edge::new(e.cursor, e.node)));
        conn
    }
}

#[cfg(test)]
mod tests {

    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct CursorData {
        content: String,
    }

    #[test]
    fn test_cursor() {
        let data = CursorData {
            content: "custom-content".into(),
        };
        let cursor = OpaqueCursor::new(&data).unwrap();

        let encoded = cursor.encode_cursor();
        assert_eq!(&encoded, "eyJjb250ZW50IjoiY3VzdG9tLWNvbnRlbnQifQ");

        let decoded = OpaqueCursor::decode_cursor(&encoded);
        assert!(decoded.is_ok(), "Could not decode the cursor");
        let inner: Result<CursorData, _> = decoded.unwrap().as_data();
        assert!(inner.is_ok(), "Could not deserialize the cursor data");
        let inner = inner.unwrap();
        assert_eq!(inner, data);
    }
}
