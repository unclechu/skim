use std::sync::Arc;
use std::thread;

use tuikit::prelude::{Event as TermEvent, Term, TermOptions, TermHeight};

use crate::event::{EventSender};
use crate::input;
use crate::options::SkimOptions;

pub struct TUI {
    term: Option<Arc<Term>>,
    input_thread: Option<thread::JoinHandle<()>>,
    tx: EventSender,

    no_mouse: bool,
    min_height: TermHeight,
    height: TermHeight,
    bind: Vec<String>,
    expect: Option<String>,
}

impl TUI {
    pub fn new(tx: EventSender, options: &SkimOptions) -> Self {
        let min_height = options
            .min_height
            .map(Self::parse_height_string)
            .expect("min_height should have default values");

        let height = options
            .height
            .map(Self::parse_height_string)
            .expect("height should have default values");

        let bind = options.bind.iter().map(|&x| String::from(x)).collect();
        let expect = options.expect.as_ref().map(|x| String::from(x));

        Self {
            term: None,
            input_thread: None,
            tx,
            no_mouse: options.no_mouse,
            min_height,
            height,
            bind,
            expect,
        }
    }

    pub fn render(mut self) {
        let term_opts = TermOptions::default()
            .min_height(self.min_height)
            .height(self.height);

        let term = Arc::new(Term::with_options(term_opts).unwrap());

        if !self.no_mouse {
            let _ = term.enable_mouse_support();
        }

        let mut input = input::Input::new();
        input.parse_keymaps(self.bind);
        input.parse_expect_keys(self.expect);

        let term_clone = term.clone();
        let tx_clone = self.tx.clone();

        let input_thread = thread::spawn(move || loop {
            if let Ok(key) = term_clone.poll_event() {
                if key == TermEvent::User1 {
                    break;
                }

                for ev in input.translate_event(key).into_iter() {
                    let _ = tx_clone.send(ev);
                }
            }
        });

        self.term = Some(term);
        self.input_thread = Some(input_thread);
    }

    pub fn with_term(self, mut cb: impl FnMut(&Arc<Term>)) {
        match self.term {
            None => (),
            Some(term) => cb(&term),
        }
    }

    pub fn finish(mut self) {
        match (self.term, self.input_thread) {
            (Some(term), Some(input_thread)) => {
                // interrupt the input thread
                let _ = term.send_event(TermEvent::User1);

                let _ = input_thread.join();
                self.input_thread = None;

                let _ = term.pause();
                self.term = None;
            },
            _ => ()
        }
    }

    // 10 -> TermHeight::Fixed(10)
    // 10% -> TermHeight::Percent(10)
    fn parse_height_string(string: &str) -> TermHeight {
        if string.ends_with('%') {
            TermHeight::Percent(string[0..string.len() - 1].parse().unwrap_or(100))
        } else {
            TermHeight::Fixed(string.parse().unwrap_or(0))
        }
    }
}

// if options.select_1 {
//     println!("ahoy there!");
//     std::thread::sleep(std::time::Duration::from_millis(2000));
// }
