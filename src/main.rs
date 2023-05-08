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
//!    - **\[^abx-z\]**: matches any character not in the brackets. Ranges are supported, so [^abx-z] matches any character but a, b, x, y, z
//!  - **and groups**
//!    - **\(...\)**: takes everything inside the escaped parens as a sub-regular expression. And groups can show up in the result optionally identified by a name or not, or can be hidden from the results
//!    - **\(?...\)**: a hidden group, it will not be recorded in the search results
//!    - **\(?\<NAME\>...\)**: Matches will be reported in the Report structure associated with NAME, to make it easier to find
//!  - **or groups**
//!    -**A\|B** matches either the regular expression A or the regular expression B
//!  - **repetition counts**: any expression can beexecuted multiple times by suffixing it with a repetition code 
//!    - __*__: match any number of times from 0 up
//!    - **+**: match any number of times from 1 up
//!    - **?**: match 0 or 1 repetition
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
//!         Ok(Some((path, char_start, bytes_start))) => {
//!             return Ok(Some(Report::new(&path, char_start, bytes_start).display(0)))
//!         },
//!         Ok(None) => return Ok(None),
//!         Err(error) => Err(error),
//!     }
//! }
///
/// In addition, there is an alternative style regular expression syntax supported. As I was writing the original parser I noticed
/// a few things: first, the main difficulty of the sntax was in handling special cases required for the quirky traditional syntax
/// (examples: infix operator for **or**, characters distributing individually instead of in a group, naming only supported for
/// **and** nodes, ... I also noticed an elisp program that provided macros to make ubderstanding long, complicated REs easier.
/// Putting this together I designed a simpler regexp syntax, which made writing a parser to support it much simpler. The basic syntax:
///
/// - There are 3 kinds of nodes: **AND** nodes, **OR** nodes, and **CHAR** nodes.
///   - **AND** nodes are created by using the syntax *"and(...)"
///   - **OR** nodes are created by using the syntax *"or(...)"
///   - **CHAR** nodes can be created in a few ways:
///     - **txt(...)** : explicitly writing the node function
///     - **'TEXT...'** or **"TEXT..."** : wrapping the text in single or double quotes
///     - TEXT... : any text entered that is not recognized is assumed to be text. This can be a little
///          tricky though: first, spaces are seen as separaters rather than characters, so if text contains
///          whitespace it must use quotes or the function form. Second, there must be a terminating space. For
///          example, *and( abc)* is not an and with the text node "abc" but is instead an incomplete **AND** node
///          containing a text node "abc)"
///   - Repetition is handled like traditional REs, the characters *, +, ?, and {} can trail any unit and have the same meanings
///   - Naming: Names are assigned to any unit by trailing it with the notation *"<NAME>"*. Names combine with repetition
///     counts,As an example, __and("ab")<name>*__ matches the string "abab", with named strings 'name: "ab"'
///     and 'name: "ab"', while __and("ab")*<name>__ matches the string "abab" once with string 'name: "abab".
///   - Definitions: the alternative syntax also allows defining substrings that can get reinserted multiple times
///         in a single regular expression. They are defined using the function syntax def(name: RE), which can appear
///         either in the RE itself or in a file included in the regexp session using the **use(filaneme)** command

pub mod regexp;
mod interactive;

//use crate::regexp;
//use std::env;
use crate::regexp::Report;
use crate::interactive::Interactive;

use core::sync::atomic::{AtomicIsize, AtomicUsize, AtomicU32, Ordering::{Acquire, Release, AcqRel}};

use clap::{Parser, value_parser};               // Command Line Argument Processing

/// default value for the **--Abbrev** switch
const ABBREV_DEFAULT: u32 = 5;
/// default value for the **--alt** switch
const PARSER_DEFAULT: &str = "traditional";

/// value for tab size: the number of spaces to indent for each level
const TAB_SIZE:usize = 4;

// TODO: make trace a macro?
/// the debug levelthe program is running under
static TRACE_LEVEL: AtomicUsize = AtomicUsize::new(0);
/// the number of indents to print before nested trace lines
static TRACE_INDENT: AtomicIsize = AtomicIsize::new(0);

fn set_trace(level: usize) {
    TRACE_LEVEL.store(level, Release)
}

/// **trace()** is used to control output of debug information, and also to view steps in the walk phase. It uses a static mut value in order to be available everywhere. 
pub(crate) fn trace(level: usize) -> bool {
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

/// the number of indents to print before nested trace lines
static ABBREV_LEN: AtomicU32 = AtomicU32::new(5);
fn set_abbrev(len: u32) {
    ABBREV_LEN.store(len, Release)
}
pub(crate) fn get_abbrev() -> usize {
    usize::try_from(ABBREV_LEN.load(Acquire)).unwrap_or_default()
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
    /// Parser to use. Will accept abbreviations. Currently supported are 'traditional' and 'alternative'.
    #[clap(short, long, default_value_t = String::from(PARSER_DEFAULT))]
    pub parser: String, 
    /// Start up an interactive session
    #[clap(short, long, default_value_t = false)]
    pub interactive: bool,
    /// Prints the parsed regexp tree
    #[clap(short, long, default_value_t = false)]
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
    // length of text to display in the --debug output
    /// When tracing the walk phase abbreviate the display of current string to LENGTH chars
    #[clap(short, long, default_value_t = ABBREV_DEFAULT, value_parser=value_parser!(u32).range(1..))]
    pub abbrev: u32,
}

impl Config {
    /// Reads the command line information and performs some cross-member checks difficult to do in *clap*. This returns
    /// a _Config_ instance whose members provide the desired values, or an error if the values are not allowed.
    fn load() -> Result<Config, &'static str> {
        let config = Config::parse();
        if !"alternative".starts_with(&config.parser) && ! "traditional".starts_with(&config.parser) {
            Err("Choices for parser are 'traditional' or 'alternative'")
        } else if config.interactive { Ok(config) }
        else if config.re.is_empty() {
            Err("RE is required unless --interactive given")
        } else if config.text.is_empty() && !config.tree {
            Err("TEXT is required unless --interactive or --tree given")
        } else {Ok(config)}
    }

    pub fn alt_parser(&self) -> bool { "traditional".starts_with(&self.parser) }
        
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
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(msg) => {
            println!("{}", msg);
            return;
        }
    };
    set_abbrev(config.abbrev);
    
    if config.interactive { return Interactive::new(config).run(); }
    set_trace(config.debug as usize);
    // execution starts
    match regexp::parse_tree(&config.re, "alternative".starts_with(&config.parser)) {
        Err(error) => println!("{}", error),
        Ok(tree) => {
            if config.tree {
                println!("--- Parse tree:");
                tree.desc(0);
            }
            //println!("{:?}", config);
            if !config.text.is_empty() {
                match regexp::walk_tree(&tree, &config.text) {
                    Ok(Some(path)) => {
                        if config.walk {
                            println!("--- Walk:");
                            path.dump(0);
                            println!("--- End walk");
                        }
                        let report = Report::new(&path);
                        report.display(0);
                        if config.named {
                            for (name, v) in report.get_named() {
                                if v.len() == 1 { println!("{}: \"{}\"", if name.is_empty() {"(unnamed)"} else {name}, v.last().unwrap().string()); }
                                else {
                                    println!("{}: ", if name.is_empty() {"(unnamed)"} else {name});
                                    v.iter().for_each(|x| println!("    \"{}\"", x.string()));
                                }
                            }
                        }
                    }
                    Ok(None) => println!("No match"),
                    Err(error) => println!("{}", error)
                }
            }
        }
    }
}

