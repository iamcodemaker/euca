//! Traits to implement on a model to allow it to interact with an application.

/// Process a message that updates the model.
pub trait Update<Message, Command> {
    /// Update the model using the given message.
    fn update(&mut self, msg: Message, commands: &mut Vec<Command>);
}

/// Render (or view) the model as a virtual dom.
pub trait Render<DomTree> {
    /// Render the model as a virtual dom.
    fn render(&self) -> DomTree;
}