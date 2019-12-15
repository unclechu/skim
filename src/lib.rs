#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
mod ansi;
mod event;

use std::borrow::Cow;
use std::env;
use std::fmt::Display;
use std::io::BufRead;
use std::io::BufReader;
use std::os::unix::io::AsRawFd;
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::thread;
use crate::event::Event;

//==============================================================================
/// How to preview an item? Invoking a command or use provided content.
pub enum Preview<'a> {
    Command(&'a str),
    Provided(&'a str),
}

pub trait SkimItem: Send + Sync {
    /// return the raw content of an item
    fn get_raw(&self) -> Cow<str>;
    /// define the content to preview
    fn get_preview(&self) -> Option<Preview>;
}

//==============================================================================
/// reader: the one that provides items. The protocol is:
/// - when `start` was called, reader start to collecting items.
/// - every now and then, `take` was called by skim to take all the collected items.
/// - every now and then `is_done` will be called to check if the reading process is done or not,
///   so this function is better to be fast
/// - when skim is about to quit or restart, `stop` will be called
trait Reader {
    fn stop(&mut self);
    fn start(&mut self, command: &str, query: &str);
    fn take(&mut self) -> Vec<Arc<dyn SkimItem>>;
    fn is_done(&self) -> bool;
}

//==============================================================================
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum MatchedRange {
    ByteRange(usize, usize), // range of bytes
    Chars(Vec<usize>),       // individual character indices matched
}

/// try to match an item, return its score and the matching indices
///
/// Should implement `Sync` and `Send`, so that it could be use across threads.
pub trait Matcher: Sync + Send {
    /// Matcher is responsible for matching lots of items against one query
    /// So it is better to do some preparation for it first and then match.
    fn compile(&mut self, query: &str);

    /// match the text and return its score and matching indices(of character)
    fn match_item(&self, text: &str) -> Option<(i64, MatchedRange)>;
}

//==============================================================================
pub trait SkimOutput {
    fn query(&self) -> &str;
    fn cmd_query(&self) -> &str;
    fn selections(&self) -> Arc<dyn SkimItem>;
    fn current_cursor(&self) -> Arc<dyn SkimItem>;
    fn last_event(&self) -> Event;
}

//==============================================================================
pub struct Skim {}
