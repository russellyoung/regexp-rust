#![allow(dead_code)]
mod regexp;

//use crate::regexp;
//use std::env;
use clap::{Parser, value_parser};               // Command Line Argument Processing

// interactive mode (TODO)
const INTERACTIVE_DEFAULT: bool = false;
// print RE parse tree
const PRINTTREE_DEFAULT: bool = false;
// print debugging messages
const DEBUG_DEFAULT: u32 = 0;
const ABBREV_DEFAULT: u32 = 5;

const TAB_SIZE:usize = 2;                     // indent in Debug display

// Used for debugging: the function trace(Path) either is a no-op or prints the given path, depending on the command line args
//static mut trace = |x| { println!("{:#?}", x) };
// TODO: make this a macro
static mut TRACE_LEVEL: u32 = 0;
fn set_trace(level: u32) { unsafe { TRACE_LEVEL = level }}
pub fn trace(level: u32) -> bool { unsafe { level <= TRACE_LEVEL }}

static mut TRACE_INDENT:isize = 0;
// when assigning trace levels to print statements make sure lines that change the indent level have the same trace level
pub fn trace_change_indent(delta: isize) { unsafe {TRACE_INDENT += delta; }}
pub fn trace_indent() -> String { unsafe { pad(TRACE_INDENT as usize) }}

// helper function to format debug
fn pad(x: usize) -> String {
    let pad = TAB_SIZE*x;
    format!("{:pad$}", "")
}

/// rer (regular Expressions Rust): sample Rust program to search strings using regular expressions
/// similar to (but not identical to) elisp regular expressions (which is also similar to perl
/// regular expressions).
/// 	
/// The search has two phases, in the first phase it parses the regexp to get a regexp tree, and in the
/// second it walks the tree trying to find a path covering all the nodes.
///
/// The basic regular expression syntax is like elisp:
///  - non-special characters match themselves
///  - special characters:
///    - ^ (only at front of RE): matches the beginning of the string
///    - $ (only at end of RE): matches the end of the string
///    - .: matches everything
///    - \N: matches digits
///  - ranges: [abx-z] matches any character in the brackets. Ranges are supported, so the previous range matches any of a, b, x, y, z
///  - not ranges: [^ab] matches any character not in the brackets. Ranges are supported, so [^abx-z] matches any character but a, b, x, y, z
///  - and groups: \(...\) takes everything inside the escaped parens as a sub-regular expression.
///  - or groups: A\|B matches either the regular expression A or the regular expression B
///
/// In addition, any unit can be modified by following it with a repetition code. The codes are:
///  - *: match any number of times from 0 up
///  - +: match any number of times from 1 up
///  - ?: match 0 or 1 repitition
///  - {N}: match exactly N times
///  - {N,}: match N or more times
///  - {N,M}: match any number of repititions from M to N
///
/// By default this uses a greedy search algorithm: it always matches as many times as possible and backs off if needed.
/// Any repetition code can be directed to use a lazy algorithm by suffixing it with '?'. (ie "*?, +?, ??, etc.) Lazy
/// evaluation evaluates it th esmallest number of times and then adds a new step if the first path does not complete.

#[derive(Parser, Debug)]
#[command(author, version, about, verbatim_doc_comment)]
pub struct Config {
    /// Regular expression to search for (required)
    #[clap()]
    pub re: String,
    /// String to search (required, unless --tree is used)
    #[clap(default_value_t = String::from(""))]
    pub text: String,
    /// Start up an interactive session (TODO)
    #[clap(short, long, default_value_t = INTERACTIVE_DEFAULT)]
    pub interactive: bool,
    /// Prints the parsed regexp tree
    #[clap(short, long, default_value_t = PRINTTREE_DEFAULT)]
    pub tree: bool,
    /// Prints debug information during the WALK phase. 1 - 4 give progressively more data
    #[clap(short, long, default_value_t = DEBUG_DEFAULT, value_parser=value_parser!(u32).range(0..40))]
    pub debug: u32,
    // length of text to display in the --debug output
    #[clap(short, long, default_value_t = ABBREV_DEFAULT, value_parser=value_parser!(u32).range(1..))]
    pub abbrev: u32, 
}

impl Config {
    fn get() -> Result<Config, &'static str> {
        let config = Config::parse();
        // custom checks go here
        if config.text.is_empty() && !config.tree {
            Err("Either -t (show parse tree) or TEXT is required")
        } else {Ok(config)}
    }
}

fn main() {
    let config = match Config::get() {
        Ok(cfg) => cfg,
        Err(msg) => {
            println!("{}", msg);
            return;
        }
    };
    set_trace(config.debug);
    crate::regexp::walk::set_abbrev_size(config.abbrev);
    // execution starts
    let tree = match regexp::parse_tree(&config.re) {
        Ok(node) => node,
        Err(error) => {
            println!("{}", error);
            return;
        },
    };
    if config.tree {
        println!("--- Parse tree:\n{:?}", tree);
    }
    if !config.text.is_empty() {
        let result = regexp::walk_tree(&tree, &config.text);
        match result {
            Ok(Some(path)) => { if let Some(report) = path.report() { report.display(0); } else {println!("{}", path.matched_string());} },
            Ok(None) => println!("No match"),
            Err(error) => println!("{}", error)
        }
    }
}
