use std::fmt;

/// Trait to identify authenticated subjects
pub trait Subject: Clone + fmt::Display + Send + Sync + 'static {}

/// Trait to identify subjects with roles
pub trait RoleBasedSubject: Subject {
    /// Retrieves the list of roles for the subject
    fn roles(&self) -> &[String];
}
