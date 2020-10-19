use wasm_bindgen::prelude::*;
use cfg_if::cfg_if;
use euca::app::*;
use euca::dom::*;


/// Our model is just an i32.
struct Model(i32);

impl Model {
    fn new() -> Self {
        Model(0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    Increment,
    Decrement,
}

impl Update<Msg> for Model {
    fn simple_update(&mut self, msg: Msg) {
        match msg {
            Msg::Increment => self.0 += 1,
            Msg::Decrement => self.0 -= 1,
        }
    }
}

fn button(text: &str, msg: Msg) -> Dom<Msg> {
    Dom::elem("button")
        .event("click", msg)
        .push(text)
}

fn counter(count: i32) -> Dom<Msg> {
    Dom::elem("div")
        .push(count.to_string())
}

impl Render<DomVec<Msg>> for Model {
    fn render(&self) -> DomVec<Msg> {
        vec![
            button("+", Msg::Increment),
            counter(self.0),
            button("-", Msg::Decrement),
        ].into()
    }
}

cfg_if! {
    if #[cfg(feature = "console_error_panic_hook")] {
        fn set_panic_hook() {
            console_error_panic_hook::set_once();
        }
    }
    else {
        fn set_panic_hook() {}
    }
}

cfg_if! {
    if #[cfg(feature = "console_log")] {
        fn init_log() {
            console_log::init_with_level(log::Level::Trace)
                .expect("error initializing log");
        }
    }
    else {
        fn init_log() {}
    }
}

/// This will get exported as the `default()` function for this module.
#[wasm_bindgen(start)]
pub fn run() -> Result<(), JsValue> {
    set_panic_hook();
    init_log();

    let parent = web_sys::window()
        .expect("couldn't get window handle")
        .document()
        .expect("couldn't get document handle")
        .query_selector(".app")
        .expect("error querying for element")
        .expect("expected <section class=\"app\"></section>");

    let _ = AppBuilder::default()
        .attach(parent, Model::new());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // we can test the model in isolation by initializing it, then sending it the messages we want,
    // and checking that it's state is as expected. This can be done by checking individual
    // elements in the model or if the model implements PartialEq, we can check the whole model at
    // once.
    #[test]
    fn increment() {
        let mut model = Model::new();
        model.update(Msg::Increment, &mut Commands::default());
        assert_eq!(model.0, 1);
    }

    #[test]
    fn decrement() {
        let mut model = Model::new();
        model.update(Msg::Decrement, &mut Commands::default());
        assert_eq!(model.0, -1);
    }

    // we can also test the view/renering code by sending it a model and checking the dom that
    // comes out. This requires a custom PartialEq implementation and a custom Debug implementation
    // that ignores web_sys nodes and closures as those don't have PartialEq or Debug. DomItem has
    // PartialEq and Debug implementations that meet this criteria, so we can implement comparisons
    // for testing purposes in terms of the dom iterator.
    #[test]
    fn basic_render() {
        let model = Model::new();
        let dom = model.render();

        let reference: DomVec<Msg> = vec![
            button("+", Msg::Increment),
            counter(0),
            button("-", Msg::Decrement),
        ].into();

        // here we could do this
        //
        // ```rust
        // assert!(dom.dom_iter().eq(reference.dom_iter()));
        // ```
        //
        // but we want to use assert_eq!() so we can see the contents of the dom if it doesn't
        // match

        use euca::vdom::{DomIter, DomItem};
        let dom: Vec<DomItem<_, _, _>> = dom.dom_iter().collect();
        let reference: Vec<DomItem<_, _, _>> = reference.dom_iter().collect();
        assert_eq!(dom, reference);
    }

    // we can also use this technique to test individual dom generation components instead of
    // testing the entire render function if necessary
}
