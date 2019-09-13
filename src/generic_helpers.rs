//! Default implementations used for [`AppBuilder`].
//!
//! These implementations do nothing and are not actually used.
//!
//! [`AppBuilder`]: ../app/struct.AppBuilder.html

use crate::route::Route;

/// A placeholder router that does nothing and will never be used.
///
/// This serves as the default router for [`AppBuilder`] allowing apps to be constructed without
/// specifying a router.
///
/// [`AppBuilder`]: ../app/struct.AppBuilder.html
pub struct Router<Message> {
    message: std::marker::PhantomData<Message>,
}

impl<Message> Route<Message> for Router<Message> {
    fn route(&self, _url: &str) -> Option<Message> {
        None
    }
}
