#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;

use std::any::Any;
use std::borrow::Cow;
use std::fmt::Display;
use std::sync::mpsc::channel;
use std::sync::Arc;

use crossbeam::channel::{Receiver, Sender};

pub use crate::ansi::AnsiString;
pub use crate::engine::fuzzy::FuzzyAlgorithm;
use crate::event::{EventReceiver, EventSender};
pub use crate::item::{ItemWrapper, MatchedItem};
use crate::model::Model;
use crate::tui::TUI;
pub use crate::options::SkimOptions;
pub use crate::output::SkimOutput;
use crate::reader::Reader;

mod ansi;
mod engine;
mod event;
mod field;
mod header;
mod input;
mod item;
mod item_collector;
mod matcher;
mod model;
mod tui;
mod options;
mod orderedvec;
mod output;
pub mod prelude;
mod previewer;
mod query;
mod reader;
mod selection;
mod spinlock;
mod theme;
mod util;

//------------------------------------------------------------------------------
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any> AsAny for T {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A `SkimItem` defines what's been processed(fetched, matched, previewed and returned) by skim
///
/// # Downcast Example
/// Normally skim will return the item back, but in `Arc<dyn SkimItem>`. You might want a reference
/// to the concrete type instead of trait object. Skim provide a somehow "complicated" way to
/// `downcast` it back to the reference of the original concrete type.
///
/// ```rust
/// use skim::prelude::*;
///
/// struct MyItem {}
/// impl SkimItem for MyItem {
///     fn display(&self) -> Cow<AnsiString> {
///         unimplemented!()
///     }
///
///     fn text(&self) -> Cow<str> {
///         unimplemented!()
///     }
/// }
///
/// impl MyItem {
///     pub fn mutable(&mut self) -> i32 {
///         1
///     }
///
///     pub fn immutable(&self) -> i32 {
///         0
///     }
/// }
///
/// let mut ret: Arc<dyn SkimItem> = Arc::new(MyItem{});
/// let mutable: &mut MyItem = Arc::get_mut(&mut ret)
///     .expect("item is referenced by others")
///     .as_any_mut() // cast to Any
///     .downcast_mut::<MyItem>() // downcast to (mut) concrete type
///     .expect("something wrong with downcast");
/// assert_eq!(mutable.mutable(), 1);
///
/// let immutable: &MyItem = (*ret).as_any() // cast to Any
///     .downcast_ref::<MyItem>() // downcast to concrete type
///     .expect("something wrong with downcast");
/// assert_eq!(immutable.immutable(), 0)
/// ```
pub trait SkimItem: AsAny + Send + Sync + 'static {
    /// The content to be displayed on the item list, could contain ANSI properties
    fn display(&self) -> Cow<AnsiString>;

    /// the string to be used for matching(without color)
    fn text(&self) -> Cow<str>;

    /// Custom preview content, default to `ItemPreview::Global` which will use global preview
    /// setting(i.e. the command set by `preview` option)
    fn preview(&self) -> ItemPreview {
        ItemPreview::Global
    }

    /// Get output text(after accept), default to `text()`
    /// Note that this function is intended to be used by the caller of skim and will not be used by
    /// skim. And since skim will return the item back in `SkimOutput`, if string is not what you
    /// want, you could still use `downcast` to retain the pointer to the original struct.
    fn output(&self) -> Cow<str> {
        self.text()
    }

    /// we could limit the matching ranges of the `get_text` of the item.
    /// providing (start_byte, end_byte) of the range
    fn get_matching_ranges(&self) -> Cow<[(usize, usize)]> {
        Cow::Owned(vec![(0, self.display().stripped().len())])
    }
}

impl<T: AsRef<str> + Send + Sync + 'static> SkimItem for T {
    fn display(&self) -> Cow<AnsiString> {
        Cow::Owned(self.as_ref().into())
    }

    fn text(&self) -> Cow<str> {
        Cow::Borrowed(self.as_ref())
    }
}

//------------------------------------------------------------------------------
// Preview

pub enum ItemPreview {
    /// execute the command and print the command's output
    Command(String),
    /// Display the prepared text(lines)
    Text(String),
    /// Display the colored text(lines)
    AnsiText(String),
    /// Use global command settings to preview the item
    Global,
}

//==============================================================================
// A match engine will execute the matching algorithm

#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub enum CaseMatching {
    Respect,
    Ignore,
    Smart,
}

impl Default for CaseMatching {
    fn default() -> Self {
        CaseMatching::Smart
    }
}

pub trait MatchEngine: Sync + Send + Display {
    fn match_item(&self, item: Arc<ItemWrapper>) -> Option<MatchedItem>;
}

pub trait MatchEngineFactory {
    fn create_engine_with_case(&self, query: &str, case: CaseMatching) -> Box<dyn MatchEngine>;
    fn create_engine(&self, query: &str) -> Box<dyn MatchEngine> {
        self.create_engine_with_case(query, CaseMatching::default())
    }
}

//------------------------------------------------------------------------------
pub type SkimItemSender = Sender<Arc<dyn SkimItem>>;
pub type SkimItemReceiver = Receiver<Arc<dyn SkimItem>>;

pub struct Skim {}

impl Skim {
    pub fn run_with(options: &SkimOptions, source: Option<SkimItemReceiver>) -> Option<SkimOutput> {
        let (tx, rx): (EventSender, EventReceiver) = channel();
        let reader = Reader::with_options(&options).source(source);
        let tui = TUI::new(tx.clone(), &options);
        let mut model = Model::new(rx, tx, reader, tui, &options);
        model.start()
    }
}
