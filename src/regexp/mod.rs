pub mod tree;
pub mod walk;

// Export functions
pub use crate::regexp::tree::parse_tree;
pub use crate::regexp::walk::{walk_tree,Input};

use crate::regexp::walk::Matched;
use std::collections::HashMap;
use clap::{Parser, value_parser};               // Command Line Argument Processing
use core::sync::atomic::{AtomicIsize, AtomicUsize, Ordering::{Acquire, Release, AcqRel}};

/// General function to run a search based on the parameters in the passed Config. This can be used to simulate a grep
/// replacement. It does a search and prints out the results according to the instructions in Config. It returns the
/// number of matches found.
pub fn regexp(config: &Config) -> Result<usize, Error> {
    let mut count: usize = 0;
    let tree = parse_tree(&config.re, "alternative".starts_with(&config.parser))?;
    if config.tree {
        println!("--- Parse tree:");
        tree.desc(0);
    }
    if !config.text.is_empty() { Input::init_text(&config.text)? }
    else if !config.files.is_empty() { Input::init_files(&config.files)? }
    else { Input::init_stdin()? }
    let mut start: usize = 0;
    let match_number: usize = if config.all { 0 } else { config.count as usize };
    'main: loop {
        match walk_tree(&tree, start) {
            Err(msg) => eprintln!("{}", msg),
            Ok(None) => {
                loop {
                    match Input::next() {
                        Err(msg) => eprintln!("{}", msg),
                        Ok(false) => { break 'main; },
                        Ok(true) => { start = 0; break; },
                    }
                }
            },
            Ok(Some(path)) => {
                if config.walk {
                    println!("--- Walk:");
                    path.dump(0);
                    println!("--- End walk");
                }
                if config.quiet {
                    let (from, to) = path.range();
                    Input::apply(|input| {
                        if let Some(filename) = input.current_file() {
                            println!("{} ({})", &input.full_text[from..to], filename);
                        } else {
                            println!("{}", &input.full_text[from..to]);
                        }
                    });
                } else {
                    let report = Report::new(&path);
                    report.display(0);
                    if config.named {
                        Input::apply(|input| {
                            for (name, v) in report.get_named() {
                                if v.len() == 1 { println!("{}: \"{}\"", if name.is_empty() {"(unnamed)"} else {name}, v[0].string(input)); }
                                else {
                                    println!("{}: ", if name.is_empty() {"(unnamed)"} else {name});
                                    v.iter().for_each(|x| println!("    \"{}\"", x.string(input)));
                                }
                            }
                        });
                    }
                }
                count += 1;
                start = path.end();
                    if count == match_number { break; }
            }
        }
    }
    Ok(count)
}

/// search strings using either traditional regular expressions or in a new (better) syntax
/// default value for the **--Abbrev** switch
const ABBREV_DEFAULT: u32 = 5;
/// default value for the **--alt** switch
const PARSER_DEFAULT: &str = "traditional";

/// Config is used to parse command line arguments and pass them to functions that perform the search
#[derive(Parser, Debug)]
#[command(author, version, about, verbatim_doc_comment)]
pub struct Config {
    /// Regular expression to search for (required unless --interactive)
    #[clap(default_value_t = String::from(""))]
    pub re: String,
    /// Files to search, file to search
    #[clap(num_args=0..)]
    pub files: Vec<String>, 
    #[clap(short, long, default_value_t = String::from(""))]
    pub text: String,
    /// Parser to use. Will accept abbreviations. Currently supported are 'traditional' and 'alternative'.
    #[clap(short, long, default_value_t = String::from(PARSER_DEFAULT))]
    pub parser: String,
    /// Start up an interactive session
    #[clap(short, long, default_value_t = false)]
    pub interactive: bool,
    /// Prints the parsed regexp tree
    #[clap(short('T'), long, default_value_t = false)]
    pub tree: bool,
    /// Dumps the current path (the successful path, if called on the result of walk())
    #[clap(short, long, default_value_t = false)]
    pub walk: bool,
    /// Prints debug information. 1 - 8 give progressively more data
    #[clap(short, long, default_value_t = 0, value_parser=value_parser!(u32).range(0..40))]
    pub debug: u32,
    /// Prints result for all named units
    #[clap(short, long, default_value_t = false, )]
    pub named: bool,
    /// find all instances instead of just first
    #[clap(short,long,default_value_t = false)]
    pub all: bool,
    /// number of matches to find. Overruled by --all if it appears
    #[clap(short, long, default_value_t = 1)]
    pub count: u32,
    /// just print out matched strings, no details or names
    #[clap(short,long,default_value_t = false)]
    pub quiet: bool,
}

impl Config {
    /// Reads the command line information and performs some cross-member checks difficult to do in *clap*. This returns
    /// a _Config_ instance whose members provide the desired values, or an error if the values are not allowed.
    pub fn load() -> Result<Config, &'static str> {
        let config = Config::parse();
        if !"alternative".starts_with(&config.parser) && ! "traditional".starts_with(&config.parser) {
            Err("Choices for parser are 'traditional' or 'alternative'")
        } else if config.interactive {
            if !config.files.is_empty() { Err("FILE cannot be specified for interactive run") }
            else if config.tree { Err("TREE cannot be specified for interactive run") }
            else { Ok(config) }
        }
        else if config.re.is_empty() {
            Err("RE is required unless --interactive given")
        } else if !config.text.is_empty() && !config.files.is_empty() {
            Err("FILE cannot be given if search text is passed in")
        }
        else {Ok(config)}
    }
    /// returns TRUE if the argument directs using the alternative parser, FALSE to use the traditional one
    pub fn alt_parser(&self) -> bool { "traditional".starts_with(&self.parser) }
        
}

//////////////////////////////////////////////////////////////////
//
// Report
//
/// Used to deliver the search results to the caller. Results form a tree, AndNode and OrNode are branches, the other
/// Nodes are leaves. **Report** is built up from the successful **Path** that walked the entire tree.
//
//////////////////////////////////////////////////////////////////

#[derive(Debug,Clone)]
pub struct Report {
    /// Match information for the step this Report is representing
    pub matched: Matched,
    /// The name of the field: if None then the field should not be included in the Report tree, if Some("") it is included but
    /// unnamed, otherwise it is recorded with the given name
    pub name: Option<String>,
    /// Array of child Report structs, only non-empty for And and Or nodes. OrNodes will have only a single child node, AndNodes can have many.
    pub subreports: Vec<Report>,
}

impl<'a> Report {
    /// Constructor: creates a new report from a successful Path
    pub fn new(root: &'a crate::walk::Path) -> Report {
        let mut reports = root.gather_reports();
        let mut ret = reports.splice(0.., None);
        ret.next().unwrap()
    }

    // API accessor functions
    /// Gets the string matched by this unit
    /// This is intended to be used inside an Input::apply() block, which is how to get the Input object
    pub fn string<'b>(&'b self, input: &'b Input) -> &'b str { &input.full_text[self.matched.start..self.matched.end] }

    /// Gets **Report** nodes representing matches for named Nodes. The return is a *Vec* because named matches can occur multiple
    /// times - for example, _\?\<name\>abc\)*_
    pub fn get_by_name<'b>(&'b self, name: &'b str) -> Vec<&Report> {
        let mut v = Vec::<&Report>::new();
        if let Some(n) = &self.name {
            if n == name { v.push(self); }
        }
        for r in &self.subreports {
            let mut x = r.get_by_name(name);
            v.append(&mut x);
        }
        v
    }

    /// Gets a hash of  **Report** nodes grouped by name. This just sets things up and calls **get_named_internal()** to do the work
    pub fn get_named(& self) -> HashMap<&str, Vec<&Report>> {
        let hash = HashMap::new();
        self.get_named_internal(hash)
    }

    /// internal function that does the work for **get_named()**
    fn get_named_internal<'b: 'a>(&'b self, mut hash: HashMap<&'b str, Vec<&'b Report>>) -> HashMap<&'b str, Vec<&Report>> {
        if let Some(name) = &self.name {
            if let Some(mut_v) = hash.get_mut(&name.as_str()) {
                mut_v.push(self);
            } else {
                hash.insert(name.as_str(), vec![self]);
            }
            for r in self.subreports.iter() {
                hash = r.get_named_internal(hash);
            }
        }
        hash
    }

    /// Gets the start and end position of the match in bytes
    pub(crate) fn byte_pos(&self) -> (usize, usize) { (self.matched.start, self.matched.end) }
    /// Gets the start and end position of the match in chars
    pub(crate) fn char_pos(&self) -> (usize, usize) { (self.matched.char_start, self.matched.char_start + self.matched.len_chars()) }
    /// Gets the length of the match in bytes
    pub(crate) fn len_bytes(&self) -> usize { self.matched.len_bytes() }
    /// Gets the length of the match in chars
    pub(crate) fn len_chars(&self) -> usize { self.matched.len_chars() }
//    pub fn full_string(&self) -> &str { self.matched.full_string }
    /// Pretty-prints a report with indentation to help make it easier to read
    pub(crate) fn display(&self, indent: usize) {
        let name_str = { if let Some(name) = &self.name { format!("<{}> ", name) } else { "".to_string() }};
        print!("{0:1$}", "", indent);
        let len_chars = self.matched.len_chars();
        let file_str = Input::apply(|input| { if let Some(filename) = input.current_file() { format!(": {}", filename) } else { "".to_string() }});
        Input::apply(|input| 
                     println!("\"{}\" {}chars start {}, length {}; bytes start {}, length {}{}",
                              &input.full_text[self.matched.start..self.matched.end],
                              name_str,
                              self.matched.char_start,
                              len_chars,
                              self.matched.start,
                              self.matched.end - self.matched.start,
                              file_str));
        self.subreports.iter().for_each(move |r| r.display(indent + TAB_SIZE));
    }

}

/// value for tab size: the number of spaces to indent for each level
pub const TAB_SIZE:usize = 4;

/// the debug levelthe program is running under
pub static TRACE_LEVEL: AtomicUsize = AtomicUsize::new(0);
/// the number of indents to print before nested trace lines
pub static TRACE_INDENT: AtomicIsize = AtomicIsize::new(0);

pub(crate) fn set_trace(level: usize) {
    TRACE_LEVEL.store(level, Release)
}

/// **trace()** is used to control output of debug information, and also to view steps in the walk phase. It uses a static mut value in order to be available everywhere. 
pub(crate) fn trace_level(level: usize) -> bool {
    level <= TRACE_LEVEL.load(Acquire)
}

/// **trace_change_indent()** is used to increase or decrease the current trace indent level
pub(crate) fn trace_change_indent(delta: isize) {
    TRACE_INDENT.fetch_add(delta, AcqRel);
}
/// **trace_set_indent()** is used to reset the indent level to a desired value, usually 0
pub(crate) fn trace_set_indent(size: isize) {
    TRACE_INDENT.store(size, Release);
}
pub(crate) fn trace_get_indent() -> usize {
    usize::try_from(TRACE_INDENT.load(Acquire)).unwrap_or_default() * TAB_SIZE
}
/// ** trace_indent()** gets the number of spaces to use as prefix to trace output
pub(crate) fn trace_indent() {
    print!("{0:1$}", "", trace_get_indent());
}

/// simple struct used to provide control on how errors are displayed
/// Binding messages with numbers makes testing easier
#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub code: usize,
}

impl Error {
    /// constructor
    pub fn make(code: usize, msg: &str,) -> Error { Error{code, msg: msg.to_string()}}
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Error:{}: {}", self.code, self.msg)
    }
}

/// If LEVEL >= current debug level then print out the remaining arguments in println!()-like fashion
#[macro_export]
macro_rules! trace {
    ( $level:expr, $($arg:tt)*) => {
        #[allow(unused_comparisons)]   // pass 0 as level to print a message, his suppresses the warning
        if $level <= $crate::TRACE_LEVEL.load(core::sync::atomic::Ordering::Acquire) { trace_indent(); println!($($arg)*); }
    }
}

/// If LEVEL >= current debug level adjust the indent level by adding DELTA to it
#[macro_export]
macro_rules! trace_change_indent {
    ( $level:expr, $delta:expr) => {
        #[allow(unused_comparisons)]
        if $level <= $crate::TRACE_LEVEL.load(core::sync::atomic::Ordering::Acquire) { $crate::TRACE_INDENT.fetch_add($delta, core::sync::atomic::Ordering::AcqRel); }
    }
}

