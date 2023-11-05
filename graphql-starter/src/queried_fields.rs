//! Utilities to include queried fields in APIs

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// This type represents the fields queried by a request.
///
/// Nested fields are allowed using a dot (`.`) separator.
/// ```ignore
/// vec![
///   "fieldOne",
///   "fieldTwo.child",
///   "fieldTwo.otherChild.field"
/// ];
/// ```
///
/// It's serialized as an optional `Vec<String>` so it can be directly used in query strings or bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueriedFields {
    /// Query all the fields available
    All,
    /// query just the listed fields
    Fields(Vec<String>),
}
impl Serialize for QueriedFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            QueriedFields::All => serializer.serialize_none(),
            QueriedFields::Fields(fields) => serializer.serialize_some(fields),
        }
    }
}
impl<'de> Deserialize<'de> for QueriedFields {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields: Option<Vec<String>> = serde::Deserialize::deserialize(deserializer)?;
        match fields {
            Some(fields) => Ok(Self::Fields(fields)),
            None => Ok(Self::All),
        }
    }
}
impl From<Option<Vec<String>>> for QueriedFields {
    fn from(value: Option<Vec<String>>) -> Self {
        match value {
            Some(fields) => QueriedFields::Fields(fields),
            None => QueriedFields::All,
        }
    }
}
impl From<QueriedFields> for Option<Vec<String>> {
    fn from(value: QueriedFields) -> Self {
        match value {
            QueriedFields::All => None,
            QueriedFields::Fields(fields) => Some(fields),
        }
    }
}
impl From<Vec<String>> for QueriedFields {
    fn from(value: Vec<String>) -> Self {
        Self::Fields(value)
    }
}
impl From<&[String]> for QueriedFields {
    fn from(value: &[String]) -> Self {
        Self::Fields(value.to_vec())
    }
}
impl From<Vec<&str>> for QueriedFields {
    fn from(value: Vec<&str>) -> Self {
        Self::Fields(value.into_iter().map(String::from).collect())
    }
}
impl From<&[&str]> for QueriedFields {
    fn from(value: &[&str]) -> Self {
        Self::Fields(value.iter().map(|&s| s.into()).collect())
    }
}

impl QueriedFields {
    /// Returns wether every field is being queried or not
    pub fn all_fields_queried(&self) -> bool {
        match self {
            QueriedFields::All => true,
            QueriedFields::Fields(_) => false,
        }
    }

    /// Checks wether no fields are being queried
    pub fn is_empty(&self) -> bool {
        match self {
            QueriedFields::All => false,
            QueriedFields::Fields(f) => f.is_empty(),
        }
    }

    /// Checks if a given field is being queried.
    ///
    /// ## Examples:
    /// ``` rust
    /// # use graphql_starter::queried_fields::QueriedFields;
    /// let query = QueriedFields::from(vec!["a", "a.b.c", "a.b.d", "b", "c.d"]);
    ///
    /// assert!(query.contains("a"));
    /// assert!(query.contains("a.b"));
    /// assert!(!query.contains("d"));
    /// ```
    pub fn contains(&self, field: &str) -> bool {
        match self {
            QueriedFields::All => true,
            QueriedFields::Fields(fields) => {
                let prefix = format!("{field}.");
                fields.iter().any(|f| f == field || f.starts_with(&prefix))
            }
        }
    }

    /// Returns the [QueriedFields] for a given child field.
    ///
    /// ## Examples:
    /// ``` rust
    /// # use graphql_starter::queried_fields::QueriedFields;
    /// let query = QueriedFields::from(vec!["a", "a.b.c", "a.b.d", "b", "c.d"]);
    ///
    /// assert!(query.child("d").is_empty());
    /// assert!(query.child("a.b").contains("c"));
    /// assert!(query.child("a.b").contains("d"));
    /// ```
    pub fn child(&self, field: &str) -> QueriedFields {
        match self {
            QueriedFields::All => QueriedFields::All,
            QueriedFields::Fields(fields) => {
                if fields.is_empty() {
                    QueriedFields::Fields(Vec::default())
                } else {
                    let prefix = format!("{field}.");
                    QueriedFields::Fields(
                        fields
                            .clone()
                            .into_iter()
                            .filter_map(|s| s.strip_prefix(&prefix).map(|stripped| stripped.to_string()))
                            .collect(),
                    )
                }
            }
        }
    }

    /// Returns the [QueriedFields] for the [Edge's](crate::pagination::Edge) `node` in a
    /// [Page](crate::pagination::Page).
    ///
    /// ## Examples:
    /// ``` rust
    /// # use graphql_starter::queried_fields::QueriedFields;
    /// let query = QueriedFields::from(vec![
    ///     "pageInfo.hasNextCursor",
    ///     "pageInfo.endCursor",
    ///     "totalItems",
    ///     "edges.node.a",
    ///     "nodes.b",
    /// ]);
    ///
    /// assert!(query.nodes().contains("a"));
    /// assert!(query.nodes().contains("b"));
    /// ```
    pub fn nodes(&self) -> QueriedFields {
        match self {
            QueriedFields::All => QueriedFields::All,
            QueriedFields::Fields(fields) => {
                if fields.is_empty() {
                    QueriedFields::Fields(Vec::default())
                } else {
                    match (self.child("nodes"), self.child("edges").child("node")) {
                        (QueriedFields::Fields(mut nodes_fields), QueriedFields::Fields(mut edges_fields)) => {
                            nodes_fields.append(&mut edges_fields);
                            nodes_fields.sort();
                            nodes_fields.dedup();
                            QueriedFields::Fields(nodes_fields)
                        }
                        _ => QueriedFields::All,
                    }
                }
            }
        }
    }

    #[cfg(feature = "graphql")]
    /// Returns the [QueriedFields] for the [GraphQLMap's](crate::graphql::GraphQLMap)
    /// [entry](crate::graphql::GraphQLMapEntry) `values`.
    ///
    /// It's really a shortcut to `.child("value")`
    ///
    /// ## Examples:
    /// ``` rust
    /// # use graphql_starter::queried_fields::QueriedFields;
    /// let query = QueriedFields::from(vec!["a", "b.key", "b.value.c", "b.value.d"]);
    ///
    /// assert!(query.child("b").entry_values().contains("c"));
    /// assert!(query.child("b").entry_values().contains("d"));
    /// ```
    pub fn entry_values(&self) -> QueriedFields {
        self.child("value")
    }
}
