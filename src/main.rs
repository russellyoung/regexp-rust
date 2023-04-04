#![allow(dead_code)]

//! ## Regular Expression search
//! This is a sample Rust program to search strings using regular expressions
//! similar to (but not identical to) elisp regular expressions (which is also similar to perl
//! regular expressions).
//! 
//! The basic regular expression syntax is like elisp:
//!  - **non-special characters**: match themselves
//!  - **special characters**
//!    - **^** (only at front of RE): matches the beginning of the string
//!    - **$** (only at end of RE): matches the end of the string
//!    - **.**: matches everything
//!    - **\d**: matches digits
//!    - **\l**: matches lower case ascii
//!    - **\u**: matches upper case ascii
//!    - **\a**: matches ascii printable
//!    - **\n**: matches newline
//!    - **\t**: matches tab
//!  - **ranges**: matches characters in the given set
//!    - **[abx-z]** matches any character in the brackets. Ranges are supported, so the previous range matches any of a, b, x, y, z
//!  - **not ranges** matches on characters not in the given set
//!    - **[^ab]**: matches any character not in the brackets. Ranges are supported, so [^abx-z] matches any character but a, b, x, y, z
//!  - **and groups**
//!    - **\(...\)**: takes everything inside the escaped parens as a sub-regular expression. And groups can show up in the result optionally identified by a name or not, or can be hidden from the results
//!    - **\(?...\)**: a hidden group, it will not be recorded in the search results
//!    - **\(?\<NAME\>...\)**: Matches will be reported in the Report structure associated with NAME, to make it easier to find
//!  - **or groups**
//!    -**A\|B** matches either the regular expression A or the regular expression B
//!  - **repetition counts**: any expression can beexecuted multiple times by suffixing it with a repetition code 
//!    - __*__: match any number of times from 0 up
//!    - **+**: match any number of times from 1 up
//!    - **?**: match 0 or 1 repitition
//!    - **{N}**: match exactly N times
//!    - **{N,}**: match N or more times
//!    - **{N,M}**: match any number of repititions from M to N
//! 
//! By default this uses a greedy search algorithm: it always matches as many times as possible and backs off if needed.
//! Any repetition code can be directed to use a lazy algorithm by suffixing it with '?'. (ie "*?, +?, ??, etc.) Lazy
//! evaluation first matches the smalles number allowed and adds extra instances if allowed as needed.
//!
//! A search has three phases. The first phase parses the regular expression to get a regular expression tree, which is the map needed to
//! search the target string. The second phase uses the tree to walk through the target string to see if there is a match. Finally, the
//! third phase takes the Path returned by the walk phase and generates the results in a more accessible form.
//!
//! A simple example of how to use it is:
//! ```
//! fn search(regexp: &str, text: &str) -> Result<Option<Report>, String>{
//!     let tree = match regexp::parse_tree(regexp) {
//!         Ok(node) => node,
//!         Err(error) => { return Err(error); },
//!     };
//!     match regexp::walk_tree(&tree, text) {
//!         Ok(Some((path, char_start, bytes_start))) => { return Ok(Some(Report::new(&path, char_start, bytes_start).display(0))) },
//!         Ok(None) => return Ok(None),
//!         Err(error) => Err(error),
//!     }
//! }
//! ```
pub mod regexp;
mod interactive;

//use crate::regexp;
//use std::env;
use crate::regexp::Report;
use crate::interactive::Interactive;

use clap::{Parser, value_parser};               // Command Line Argument Processing

/// default value for the **--interactive** switch
const INTERACTIVE_DEFAULT: bool = false;
/// default value for the **--tree** switch
const PRINTTREE_DEFAULT: bool = false;
/// default value for the **--debug** switch
const DEBUG_DEFAULT: u32 = 0;
/// default value for the **--abbrev** switch
const ABBREV_DEFAULT: u32 = 5;

/// value for tab size: the number of spaces to indent for each level
const TAB_SIZE:isize = 4;

// TODO: make this a macro?
static mut TRACE_LEVEL: u32 = 0;
fn set_trace(level: u32) { unsafe { TRACE_LEVEL = level }}
/// **trace()** is used to control output of debug information, and also to view steps in the walk phase. It uses a static mut value in order to be available everywhere. 
pub fn trace(level: u32) -> bool { unsafe { level <= TRACE_LEVEL }}

static mut TRACE_INDENT:isize = 0;

/// **trace_change_indent()** is used to increase or decrease the current trace indent level
pub fn trace_change_indent(delta: isize) { unsafe { TRACE_INDENT += delta; } }
/// **trace_set_indent()** is used to reset the indent level to a desired value, usually 0
pub fn trace_set_indent(size: isize) { unsafe { TRACE_INDENT = size; } }
/// ** trace_indent()** gets the number of spaces to use as prefix to trace output
pub fn trace_indent() -> String { unsafe { pad(TRACE_INDENT) }}

/// A helper function to format debug. It provides strings of spaces to format output dynamically.
/// An indent level consists of the number of spaces given by TAB_SIZE,
///
/// **Args**:
///   - x: the number of levels to indent
///
/// **Returns**:
///   - String: string consisting of x * *TAB_SIZE* spaces

fn pad(x: isize) -> String {
    let pad = { if x < 0 {0} else {(TAB_SIZE*x) as usize}};
    format!("{:pad$}", "")
}

// (regular Expressions Rust): sample Rust program to search strings using regular expressions
// similar to (but not identical to) elisp regular expressions (which is also similar to perl
// regular expressions).
// The search has two phases, in the first phase it parses the regexp to get a regexp tree, and in the
// second it walks the tree trying to find a path covering all the nodes.
// 

/// A structure used to read and parse command line arguments when the program is run
#[derive(Parser, Debug)]
#[command(author, version, about, verbatim_doc_comment)]
pub struct Config {
    /// Regular expression to search for (required unless --interactive)
    #[clap(default_value_t = String::from(""))]
    pub re: String,
    /// String to search (required, unless --tree or --interactive)
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
    /// Reads the command line information and performs some cross-member checks difficult to do in *clap*. This returns
    /// a _Config_ instance whose members provide the desired values, or an error if the values are not allowed.
    fn get() -> Result<Config, &'static str> {
        let config = Config::parse();
        if config.interactive { Ok(config) }
        else if config.re.is_empty() {
            Err("RE is required unless --interactive given")
        } else if config.text.is_empty() && !config.tree {
            Err("TEXT is required unless --interactive or --tree given")
        } else {Ok(config)}
    }
}
/// Main function to run regexp as a function. It is called by
/// > cargo run [-t] [-i] [-d LEVEL] [-a LENGTH] \[REGEXP\] \[TARGET\]
/// where:
///  - **REGEXP**: a regular expression. This is always required excep if _-i_ is given
///  - **TARGET**: a string to search with *REGEXP*. It is required unless _-i_ or _-t_ are given
///  - **-t** (**--tree**): just parse the regexp to make the regexp tree and print it out in a user friendly format
///  - **-i** (**--interactive**): run an interactive session. This lets the user enter regexps and targets and run them to see the
/// results, as well as viewing the details of the tree parse or tree walk phases.
///  - **-d LEVEL** (**--debug LEVEL**): set the debug level to LEVEL. The default level is 0, good values to try are 1, 2, or 3.
///  - **-a LENGTH** (**--abbrev LENGTH**): When tracing the walk phase abbreviate the display of current string to LENGTH chars
pub fn main() {
    let config = match Config::get() {
        Ok(cfg) => cfg,
        Err(msg) => {
            println!("{}", msg);
            return;
        }
    };
    crate::regexp::walk::set_abbrev_size(config.abbrev);
    
    if config.interactive { return Interactive::new(config).run(); }
    
    set_trace(config.debug);
    // execution starts
    match regexp::parse_tree(&config.re) {
        Err(error) => println!("{}", error),
        Ok(tree) => {
            if config.tree { println!("--- Parse tree:\n{:?}", tree); }
            if !config.text.is_empty() {
                match regexp::walk_tree(&tree, &config.text) {
                    Ok(Some((path, char_start, bytes_start))) => Report::new(&path, char_start, bytes_start).display(0),
                    Ok(None) => println!("No match"),
                    Err(error) => println!("{}", error)
                }
            }
        }
    }
}

