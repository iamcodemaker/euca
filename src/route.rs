//! Router trait for generating a message when the page url changes.

/// Implement this trait on your router to allow for routing when the URL changes.
pub trait Route<Message> {
    /// Convert a new url to a message for the app.
    fn route(&self, _url: &str) -> Option<Message>;
}

/// A placeholder router that does nothing and will never be used.
///
/// This serves as the default router for [`AppBuilder`] allowing apps to be constructed without
/// specifying a router.
///
/// [`AppBuilder`]: ../app/struct.AppBuilder.html
impl<Message> Route<Message> for () {
    fn route(&self, _url: &str) -> Option<Message> {
        None
    }
}
