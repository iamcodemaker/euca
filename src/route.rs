//! Router trait for generating a message when the page url changes.

/// Implement this trait on your router to allow for routing when the URL changes.
pub trait Route<Message> {
    /// Convert a new url to a message for the app.
    fn route(_url: &str) -> Option<Message>;
}
