use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use cfg_if::cfg_if;
use log::{debug,info,error};
use euca::app::*;
use euca::route::Route;
use euca::dom;
use serde::{Serialize,Deserialize};
use serde_json;

cfg_if! {
    if #[cfg(feature = "console_error_panic_hook")] {
        #[inline]
        fn set_panic_hook() {
            console_error_panic_hook::set_once();
            debug!("panic hook set");
        }
    }
    else {
        fn set_panic_hook() {}
    }
}

cfg_if! {
    if #[cfg(feature = "console_log")] {
        #[inline]
        fn init_log() {
            console_log::init_with_level(log::Level::Trace)
                .expect("error initializing log");
            debug!("log initialized");
        }
    }
    else {
        fn init_log() {}
    }
}

const TITLE: &str = "Euca • TodoMVC";

#[derive(PartialEq)]
enum Filter {
    All,
    Active,
    Completed,
}

impl Default for Filter {
    fn default() -> Self {
        Filter::All
    }
}

#[derive(Default)]
struct Todo {
    pending_item: String,
    items: Vec<Item>,
    pending_edit: Option<(usize, String)>,
    filter: Filter,
}

impl Todo {
    fn with_items(items: Vec<Item>) -> Self {
        Todo {
            items: items,
            .. Todo::default()
        }
    }
}

#[derive(Default,Serialize,Deserialize)]
struct Item {
    #[serde(rename = "title")]
    text: String,
    #[serde(rename = "completed")]
    is_complete: bool,
}

#[derive(PartialEq,Clone,Debug)]
enum Message {
    UpdatePending(String),
    AddTodo,
    RemoveTodo(usize),
    ToggleTodo(usize),
    EditTodo(usize),
    UpdateEdit(String),
    SaveEdit,
    AbortEdit,
    ClearCompleted,
    ToggleAll,
    ShowAll(bool),
    ShowActive(bool),
    ShowCompleted(bool),
    ItemsChanged,
}

#[derive(Clone,Debug)]
enum Command {
    FocusPending,
    FocusEdit,
    PushHistory(String),
    UpdateStorage(String),
}

impl Update<Message, Command> for Todo {
    fn update(&mut self, msg: Message, cmds: &mut Commands<Command>) {
        use Message::*;

        match msg {
            UpdatePending(text) => {
                self.pending_item = text
            }
            AddTodo => {
                self.items.push(Item {
                    text: self.pending_item.trim().to_owned(),
                    .. Item::default()
                });
                self.pending_item.clear();
                self.update(ItemsChanged, cmds);
            }
            RemoveTodo(i) => {
                self.items.remove(i);
                self.update(ItemsChanged, cmds);
            }
            ToggleTodo(i) => {
                self.items[i].is_complete = !self.items[i].is_complete;
                self.update(ItemsChanged, cmds);
            }
            EditTodo(i) => {
                self.pending_edit = Some((i, self.items[i].text.clone()));
                cmds.post_render.push(Command::FocusEdit);
            }
            UpdateEdit(text) => {
                match self.pending_edit {
                    Some((_, ref mut pending_text)) => {
                        *pending_text = text;
                    }
                    _ => panic!("UpdateEdit called with no pending edit"),
                }
            }
            SaveEdit => {
                match self.pending_edit {
                    Some((i, ref text)) => {
                        if text.trim().is_empty() {
                            self.update(RemoveTodo(i), cmds);
                        }
                        else {
                            self.items[i].text = text.trim().to_owned();
                        }
                        self.pending_edit = None;
                    }
                    _ => panic!("SaveEdit called with no pending edit"),
                }
                self.update(ItemsChanged, cmds);
            }
            AbortEdit => {
                self.pending_edit = None;
            }
            ClearCompleted => {
                self.items.retain(|item| !item.is_complete);
                self.update(ItemsChanged, cmds);
            }
            ToggleAll => {
                let all_complete = self.items.iter().all(|item| item.is_complete);

                for item in self.items.iter_mut() {
                    item.is_complete = !all_complete;
                }

                self.update(ItemsChanged, cmds);
            }
            ShowAll(push_history) => {
                self.filter = Filter::All;
                if push_history {
                    cmds.push(Command::PushHistory("#/".to_owned()));
                }
            }
            ShowActive(push_history) => {
                self.filter = Filter::Active;
                if push_history {
                    cmds.push(Command::PushHistory("#/active".to_owned()));
                }
            }
            ShowCompleted(push_history) => {
                self.filter = Filter::Completed;
                if push_history {
                    cmds.push(Command::PushHistory("#/completed".to_owned()));
                }
            }
            ItemsChanged => {
                cmds.push(Command::UpdateStorage(serde_json::to_string(&self.items).unwrap()));
            }
        }
    }
}

impl SideEffect<Message> for Command {
    fn process(self, _: &Dispatcher<Message, Command>) {
        use Command::*;

        match self {
            FocusPending => {
                let pending_input = web_sys::window()
                    .expect("couldn't get window handle")
                    .document()
                    .expect("couldn't get document handle")
                    .query_selector("section.todoapp header.header input.new-todo")
                    .expect("error querying for element")
                    .expect("expected to find an input element")
                    .dyn_into::<web_sys::HtmlInputElement>()
                    .expect_throw("expected web_sys::HtmlInputElement");

                pending_input.focus().expect_throw("error focusing input");
            }
            FocusEdit => {
                let edit_input = web_sys::window()
                    .expect_throw("couldn't get window handle")
                    .document()
                    .expect_throw("couldn't get document handle")
                    .query_selector("section.todoapp section.main input.edit")
                    .expect_throw("error querying for element")
                    .expect_throw("expected to find an input element")
                    .dyn_into::<web_sys::HtmlInputElement>()
                    .expect_throw("expected web_sys::HtmlInputElement");

                edit_input.focus().expect_throw("error focusing input");
            }
            PushHistory(url) => {
                let history = web_sys::window()
                    .expect("couldn't get window handle")
                    .history()
                    .expect_throw("couldn't get history handle");

                history.push_state_with_url(&JsValue::NULL, TITLE, Some(&url)).expect_throw("error updating history");
            }
            UpdateStorage(data) => {
                let local_storage = web_sys::window()
                    .expect("couldn't get window handle")
                    .local_storage()
                    .expect("couldn't get local storage handle")
                    .expect_throw("local storage not supported?");

                local_storage.set_item("todo-euca", &data).unwrap_throw();
            }
        }
    }
}

fn read_items_from_storage() -> Vec<Item> {
    let local_storage = web_sys::window()
        .expect("couldn't get window handle")
        .local_storage()
        .expect("couldn't get local storage handle")
        .expect_throw("local storage not supported?");

    local_storage.get_item("todo-euca")
        .expect_throw("error reading from storage")
        .map_or(vec![], |items|
            match serde_json::from_str(&items) {
                Ok(items) => items,
                Err(e) => {
                    error!("error reading items from storage: {}", e);
                    vec![]
                }
            }
        )
}

impl Render<dom::DomVec<Message, Command>> for Todo {
    fn render(&self) -> dom::DomVec<Message, Command> {
        use dom::Dom;
        use dom::Handler::Event;

        let mut vec = vec![];
        vec.push(Dom::elem("header")
            .attr("class", "header")
            .push(Dom::elem("h1").push("todos"))
            .push(Dom::elem("input")
                .attr("class", "new-todo")
                .attr("placeholder", "What needs to be done?")
                .attr("autofocus", "true")
                .attr("value", self.pending_item.to_owned())
                .on("input", dom::Handler::InputValue(|s| {
                    Some(Message::UpdatePending(s))
                }))
                .on("keyup", Event(|e| {
                    let e = e.dyn_into::<web_sys::KeyboardEvent>().expect_throw("expected web_sys::KeyboardEvent");
                    match e.key().as_ref() {
                        "Enter" => Some(Message::AddTodo),
                        _ => None,
                    }
                }))
            )
        );

        // render todo list if necessary
        if !self.items.is_empty() {
            // main section
            vec.push(Dom::elem("section")
                .attr("class", "main")
                .push(Dom::elem("input")
                    .attr("id", "toggle-all")
                    .attr("class", "toggle-all")
                    .attr("type", "checkbox")
                    .attr("checked", self.items.iter().all(|item| item.is_complete).to_string())
                    .event("change", Message::ToggleAll)
                )
                .push(Dom::elem("label")
                    .attr("for", "toggle-all")
                    .push("Mark all as complete")
                )
                .push(Dom::elem("ul")
                    .attr("class", "todo-list")
                    .extend(self.items.iter()
                        .enumerate()
                        .filter(|(_, item)| {
                            match self.filter {
                                Filter::All => true,
                                Filter::Active => !item.is_complete,
                                Filter::Completed => item.is_complete,
                            }
                        })
                        .map(|(i, item)| {
                            match self.pending_edit {
                                Some((pending_i, ref pending_edit)) if pending_i == i => {
                                    item.render(i, Some(pending_edit))
                                }
                                Some(_) | None =>  {
                                    item.render(i, None)
                                }
                            }
                        })
                    )
                )
            );

            // todo footer
            vec.push({
                let remaining = self.items.iter()
                    .filter(|item| !item.is_complete)
                    .count();

                let footer = Dom::elem("footer")
                    .attr("class", "footer")
                    .push(Dom::elem("span")
                        .attr("class", "todo-count")
                        .push(Dom::elem("strong")
                            .push(remaining.to_string())
                        )
                        .push(
                            if remaining == 1 { " item left" }
                            else { " items left" }
                        )
                    )
                    .push(Dom::elem("ul")
                        .attr("class", "filters")
                        .push(Dom::elem("li")
                            .push(Dom::elem("a")
                                .attr("href", "#/")
                                .attr("class",
                                    if self.filter == Filter::All { "selected" }
                                    else { "" }
                                 )
                                .push("All")
                                .on("click", Event(|e| {
                                    e.prevent_default();
                                    Some(Message::ShowAll(true))
                                }))
                            )
                        )
                        .push(Dom::elem("li")
                            .push(Dom::elem("a")
                                .attr("href", "#/active")
                                .attr("class",
                                    if self.filter == Filter::Active { "selected" }
                                    else { "" }
                                 )
                                .push("Active")
                                .on("click", Event(|e| {
                                    e.prevent_default();
                                    Some(Message::ShowActive(true))
                                }))
                            )
                        )
                        .push(Dom::elem("li")
                            .push(Dom::elem("a")
                                .attr("href", "#/completed")
                                .attr("class",
                                    if self.filter == Filter::Completed { "selected" }
                                    else { "" }
                                 )
                                .push("Completed")
                                .on("click", Event(|e| {
                                    e.prevent_default();
                                    Some(Message::ShowCompleted(true))
                                }))
                            )
                        )
                    )
                ;
                if self.items.iter().any(|item| item.is_complete) {
                    footer.push(Dom::elem("button")
                        .attr("class", "clear-completed")
                        .push("Clear completed")
                        .event("click", Message::ClearCompleted)
                    )
                }
                else {
                    footer
                }
            });
        }

        vec.into()
    }
}

impl Item {
    fn render(&self, i: usize, pending_edit: Option<&str>) -> dom::Dom<Message, Command> {
        use dom::Dom;
        use dom::Handler::{Event,InputValue};

        let e = Dom::elem("li");

        if let Some(pending_edit) = pending_edit {
            e.attr("class", "editing")
                .push(Dom::elem("input")
                    .attr("class", "edit")
                    .attr("value", pending_edit)
                    .on("input", InputValue(|s| {
                        Some(Message::UpdateEdit(s))
                    }))
                    .event("blur", Message::SaveEdit)
                    .on("keyup", Event(|e| {
                        let e = e.dyn_into::<web_sys::KeyboardEvent>().expect_throw("expected web_sys::KeyboardEvent");
                        match e.key().as_ref() {
                            "Enter" => Some(Message::SaveEdit),
                            "Escape" => Some(Message::AbortEdit),
                            _ => None,
                        }
                    }))
                )
        }
        else {
            let e = e.push(
                Dom::elem("div")
                    .attr("class", "view")
                    .push(Dom::elem("input")
                        .attr("class", "toggle")
                        .attr("type", "checkbox")
                        .attr("checked", self.is_complete.to_string())
                        .event("change", Message::ToggleTodo(i))
                    )
                    .push(Dom::elem("label")
                        .push(self.text.to_owned())
                        .event("dblclick", Message::EditTodo(i))
                    )
                    .push(Dom::elem("button")
                        .attr("class", "destroy")
                        .event("click", Message::RemoveTodo(i))
                    )
            );

            if self.is_complete {
                e.attr("class", "completed")
            }
            else {
                e
            }
        }
    }
}

#[derive(Default)]
struct Router {}

impl Route<Message> for Router {
    fn route(&self, url: &str) -> Option<Message> {
        if url.ends_with("#/active") {
            Some(Message::ShowActive(false))
        }
        else if url.ends_with("#/completed") {
            Some(Message::ShowCompleted(false))
        }
        else {
            Some(Message::ShowAll(false))
        }
    }
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    init_log();
    set_panic_hook();

    let parent = web_sys::window()
        .expect("couldn't get window handle")
        .document()
        .expect("couldn't get document handle")
        .query_selector("section.todoapp")
        .expect("error querying for element")
        .expect("expected <section class=\"todoapp\"></section>");

    let items = read_items_from_storage();

    let app = AppBuilder::default()
        .router(Router::default())
        .attach(parent, Todo::with_items(items));

    Command::FocusPending.process(&app.into());

    info!("{} initialized", TITLE);
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use euca::app::Commands;

    #[test]
    fn add_todo() {
        let mut todomvc = Todo::default();

        todomvc.update(Message::UpdatePending("item".to_owned()), &mut Commands::default());
        todomvc.update(Message::AddTodo, &mut Commands::default());

        assert_eq!(todomvc.items.len(), 1);
        assert_eq!(todomvc.items[0].text, "item");
        assert_eq!(todomvc.items[0].is_complete, false);
    }

    #[test]
    fn remove_todo() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item::default());

        todomvc.update(Message::RemoveTodo(0), &mut Commands::default());

        assert_eq!(todomvc.items.len(), 0);
    }

    #[test]
    fn toggle_todo() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item::default());

        todomvc.update(Message::ToggleTodo(0), &mut Commands::default());

        assert_eq!(todomvc.items[0].is_complete, true);
    }

    #[test]
    fn save_edit_removes_empty() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item {
            text: "text".to_owned(),
            .. Item::default()
        });

        todomvc.update(Message::EditTodo(0), &mut Commands::default());
        todomvc.update(Message::UpdateEdit("".to_owned()), &mut Commands::default());
        todomvc.update(Message::SaveEdit, &mut Commands::default());

        assert_eq!(todomvc.items.len(), 0);
    }

    #[test]
    fn save_edit_trims_whitespace() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item {
            text: "text".to_owned(),
            .. Item::default()
        });

        todomvc.update(Message::EditTodo(0), &mut Commands::default());
        todomvc.update(Message::UpdateEdit(" edited text  ".to_owned()), &mut Commands::default());
        todomvc.update(Message::SaveEdit, &mut Commands::default());

        assert_eq!(todomvc.items.len(), 1);
        assert_eq!(todomvc.items[0].text, "edited text");
    }

    #[test]
    fn abort_edit_does_not_modify() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item {
            text: "text".to_owned(),
            .. Item::default()
        });

        todomvc.update(Message::EditTodo(0), &mut Commands::default());
        todomvc.update(Message::UpdateEdit(" edited text  ".to_owned()), &mut Commands::default());
        todomvc.update(Message::AbortEdit, &mut Commands::default());

        assert_eq!(todomvc.items.len(), 1);
        assert_eq!(todomvc.items[0].text, "text");
    }

    #[test]
    fn clear_completed() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item {
            text: "text1".to_owned(),
            .. Item::default()
        });
        todomvc.items.push(Item {
            text: "text2".to_owned(),
            is_complete: true,
            .. Item::default()
        });
        todomvc.items.push(Item {
            text: "text3".to_owned(),
            .. Item::default()
        });

        todomvc.update(Message::ClearCompleted, &mut Commands::default());

        assert_eq!(todomvc.items.len(), 2);
        assert_eq!(todomvc.items[0].text, "text1");
        assert_eq!(todomvc.items[1].text, "text3");
    }

    #[test]
    fn toggle_all() {
        let mut todomvc = Todo::default();
        todomvc.items.push(Item {
            text: "text1".to_owned(),
            .. Item::default()
        });
        todomvc.items.push(Item {
            text: "text2".to_owned(),
            is_complete: true,
            .. Item::default()
        });
        todomvc.items.push(Item {
            text: "text3".to_owned(),
            .. Item::default()
        });

        todomvc.update(Message::ToggleAll, &mut Commands::default());
        assert!(todomvc.items.iter().all(|item| item.is_complete));

        todomvc.update(Message::ToggleAll, &mut Commands::default());
        assert!(todomvc.items.iter().all(|item| !item.is_complete));
    }

    #[test]
    fn test_routes() {
        use Message::*;

        let router = Router::default();

        assert_eq!(router.route("http://localhost:8080"), Some(ShowAll(false)));
        assert_eq!(router.route("http://localhost:8080/"), Some(ShowAll(false)));
        assert_eq!(router.route("http://localhost:8080/#/"), Some(ShowAll(false)));
        assert_eq!(router.route("http://localhost:8080/#/"), Some(ShowAll(false)));
        assert_eq!(router.route("http://localhost:8080/#/active"), Some(ShowActive(false)));
        assert_eq!(router.route("http://localhost:8080/#/completed"), Some(ShowCompleted(false)));
    }

    #[test]
    fn storage_triggers() {
        use Message::*;
        use Command::*;

        let mut todomvc = Todo::default();
        todomvc.items.push(Item::default());
        todomvc.items.push(Item::default());
        todomvc.items.push(Item::default());

        // ensure the following message types generate UpdateStorage commands
        for msg in &[
            AddTodo,
            RemoveTodo(0),
            ToggleTodo(0),
            SaveEdit,
            ClearCompleted,
            ToggleAll,
            ItemsChanged,
        ] {
            // do necessary prep work
            match msg {
                SaveEdit => todomvc.update(EditTodo(0), &mut Commands::default()),
                _ => {}
            }

            let mut cmds = Commands::default();
            todomvc.update(msg.clone(), &mut cmds);

            // verify the proper commands were generated
            assert!(
                cmds.immediate.iter().any(|cmd| match cmd {
                    UpdateStorage(_) => true,
                    _ => false,
                }),
                "didn't find UpdateStorage for {:?}", msg
            );
        }
    }
}
