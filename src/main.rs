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
//! ## Alternate RE syntax

//! In addition to the standard(ish) regular expressions, there is an alternative style regular expression syntax supported.
//! While writing the original parser I noticed a few things: first, the main difficulty of the sntax was in handling special
//! cases required for the quirky traditional syntax (examples: infix operator for **or**, characters distributing individually
//! instead of in a group, naming only supported for **and** nodes, etc.)  Second, while getting my emacs setup to edit rust
//! I ran across elisp code where the writer created  macros to make understanding long, complicated REs easier.
//! Putting this together I designed a simpler regexp syntax, which made writing a parser to support it much simpler. The basic syntax:
//!
//! - There are 3 kinds of nodes: **AND** nodes, **OR** nodes, and **CHAR** nodes.
//!   - **AND** nodes search for all subnides sequentially. They are created by using the notation **and(...)**
//!   - **OR** nodes search for one of the subnodes to succeed. are created by using the syntax "**or(...)**"
//!   - **CHAR** nodes match a sring of chars or special chars explicitly.
//!     - They can be written with four different notations:
//!       - explicitly wrapping text with the **txt** tag: **txt(...)**
//!       - Wrapping the text in single quotation marks: **'...'**
//!       - Wrapping the text in double quotation marks: **"..."**
//!       - any text entered that is not included in some other tag is assumed to be text. This form, while simple, 
//!          can have unexpected behavior: first, whitespace acts to terminate a node rather than being embedded in it, so
//!          "**AB CD**" matched "_ABCD_" while "**"AB CD"**" matches "AB CD". Also, there must be a terminating space. For
//!          example, while "**and('abc')**" is an **and** node containing the single subnode "abc", "**and(abc)**" will fail
//!          to parse because it will be interpreted as an **and** node containing a **text** node "abc)", and so be missing
//!          a node terminator.
//!     - **char** nodes can contain:
//!       - regular characters: any character that has no other meaning in its context. These match exactly.
//!       - special characters (in context): there are some characters with special meaning inside definitions. These need to be
//!           escaped (using backslash) to include them in the series. These include repetition characters ('?', '*', '+', '{'),
//!           '[' used to open a range, and the terminating character, ')', '"', ''', or whitespace, depending on how the node is defined.
//!       - repetition: repetitions can be attached to individual characters and ranges inside **char** nodes simply by including the
//!           the repetition definition character(s). These refer to the single character or range preceding the repetition count, and
//!           cannot be named.
//!   - Special characters are the same as for traditional regular expressions:
//!     - **^** (only at front of RE): matches the beginning of the string
//!     - **$** (only at end of RE): matches the end of the string
//!     - **.**: matches everything
//!     - **\d**: matches digits
//!     - **\l**: matches lower case ascii
//!     - **\u**: matches upper case ascii
//!     - **\a**: matches ascii printable
//!     - **\n**: matches newline
//!     - **\t**: matches tab
//! - Repetitions are also defined the same as traditional regular expressions, but see the description of named blocks following.
//!    - Repetitions can be attached to individual characters inside **chars** nodes and to each type of node (except **char**
//!        defined using default syntax)
//!    - Repetitions are defined by:
//!      - __*__: match any number of times from 0 up
//!      - **+**: match any number of times from 1 up
//!      - **?**: match 0 or 1 repetition
//!      - **{N}**: match exactly N times
//!      - **{N,}**: match N or more times
//!      - **{N,M}**: match any number of repititions from M to N
//! - Named nodes: Like with traditional regular expressions nodes can be named, and the names used to label matched blocks
//!     of code.
//!   - Names can be assigned to any node, not just **AND** nodes like in traditional regular expressions
//!   - Names are assigned by trailing the node definition with "**\<NAME\>**".
//!   - If no name is defined for a node then matching strings are not reported individually (they are of course still reported by
//!       containing nodes). This differs from traditional regular expressions, where the default is to report all **AND** nodes
//!   - The empty name (\<\>) causes a matched block ro report itself, but without a name attached
//!   - Name definitions interact with repetition definitions. The order they are defined in is important. If the name is defined
//!       first the repetition refers to named blocks, no multiple named blocks can be returned. If the range comes first the name
//!       refers to the entire matched sequence, so a single named block will be returned. Example:
//!     - **and("abc")+\<name\>** will match the string "abcabcabc" by returning a single named "name" containing the string "abcabcabc"
//!     - **and("abc")\<name\>+** will match the string "abcabcabc" by returning 3 blocks named "name", each block containing the string "abc"
//! - Definitions: Commonly used regular expression sequences can be defined and inserted into a regular expression multiple times
//!   - A definition can be made inline by using the syntax "**def(NAME:...)**". This creates a regular expression from the "..." part
//!      that can be referred to by NAME. Definitions can be followed with block name and repetition count, which will be inherited by
//!      default by the inserted subtree
//!   - Definitions are included in a regular expression by using the "**get(NAME)**" function. If the **get** function has a name
//!      or repetition count attached to it, that will override any default values from the definition
//!   - Definitions can also be defined in a file and included by the "**use(FILENAME)**" statement
//!   - If more than one definition with the same NAME is made the last one overrides all previous ones. Evaluation is done at the
//!      end of the tree parse phase.
//!   - Definitions are evaluated recursively, so they can contain **def()** and **get()** statements. The parser checks to assure
//!       there are no loops in the definitions
//!
//! ## Usage
//! #### Command line
//! From the help:
//! 
//! Usage: regexp \[OPTIONS\] \[RE\] \[TEXT\]  
//!   
//! Arguments:  
//!   \[RE\]    Regular expression to search for (required unless --interactive) \[default: \]  
//!   \[TEXT\]  String to search (required, unless --tree or --interactive) \[default: \]  
//!   
//! Options:  
//!   -p, --parser \<PARSER\>  Parser to use. Will accept abbreviations. Currently supported are 'traditional' and 'alternative' \[default: traditional\]  
//!   -i, --interactive      Start up an interactive session  
//!   -t, --tree             Prints the parsed regexp tree  
//!   -w, --walk             Dumps the current path (the successful path, if called on the result of walk())  
//!   -d, --debug \<DEBUG\>    Prints debug information. 1 - 8 give progressively more data \[default: 0\]  
//!   -n, --named            Prints result for all named units  
//!   -a, --abbrev \<ABBREV\>  When tracing the walk phase abbreviate the display of current string to LENGTH chars \[default: 5\]  
//!   -h, --help             Print help  
//!   -V, --version          Print version  
//!
//! #### API
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
//!     match regexp::walk_tree(&tree, text, file) {
//!         Ok(Some((path, char_start, bytes_start))) => {
//!             return Ok(Some(Report::new(&path, char_start, bytes_start).display(0)))
//!         },
//!         Ok(None) => return Ok(None),
//!         Err(error) => Err(error),
//!     }
//! }
//!
//!```
//!
//! The TEXT argument to walk_tree() is the text to search, if it is non-empty. If it is empty the FILE argument gives
//! a file name to read for the input text. If both are empty, or TEXT is empty and FILE is "-" then the input is read from stdin.
//!
//! #### Interactive
//! There is also an interactive mode which allows storing of multiple regular expressions and text strings. From the help:
//! This is an interactive interface to the regexp search engine. The program keeps stacks of  
//! regular expressions and search texts and uses them to run searches. Besides simple searching  
//! the program will print out the parsed search tree and also details of the walk over the target  
//! string.  
//!   
//! Commands are in general of the form CMD \[SUBCMD \[DATA\]\], though it will try to guess the   
//! meaning of ambiguous commands. The commands and subcommands can be abbreviated with the   
//! first couple unique letters.  
//!   
//! The commands are:  
//!   - regexp:         display the current active regular expression  
//!   - regexp \[traditional | alternative\] RE:  sets a new regular expression to be the current one. The   
//!                     keyword is optional, if not given the program usually will guess what the text  
//!                     is, and ask for confirmation  
//!   - regexp history: lists the most recent regular expressions  
//!   - regexp list:    same as 're history'  
//!   - regexp NUMBER:  sets the NUMBERth item on the history list to be the current regular expression  
//!   - regular pop \[n\]:pops off (deletes) the nth re from the list. Defaults to 0 (the current RE)  
//!   - text:           displays the current active text string  
//!   - text TEXT:      sets new search text  
//!   - text list:      same as 'text history'  
//!   - text history:   lists the most recent regular expressions  
//!   - text NUMBER:    sets the NUMBERth item on the history list to be the current text  
//!   - text pop \[n\]:   pops off (deletes) the nth tex string from memory. Defauls to 0 (the current text string)  
//!   - search :        performs a RE search using the current RE and the current text.   
//!   - search NAME1 \[NAME2...\]: performs a RE search using the current RE and the current text, report only on units with the given names  
//!   - search * :      performs a RE search using the current RE and the current text, report on all named units  
//!   - search NUMBER:  performs a RE search using the current RE and the current text setting debug level to NUMBER to examine the path.  
//!                     This can be combined with search for name.  
//!   - tree \[NUMBER\]:  displays the parse tree for the current regular expression. Optional **NUMBER** sets the trace level  
//!                     to see how the parse is performed.  
//!   - help:           displays this help  
//!   - ?:              displays this help  

pub mod regexp;
mod interactive;

use crate::regexp::{Report,walk_tree};
use crate::regexp::walk::Input;
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

/// search strings using either traditional regular expressions or in a new (better) syntax

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
    /// number of matches to find
    #[clap(short, long, default_value_t = 1)]
    pub count: u32,
    
        
}

impl Config {
    /// Reads the command line information and performs some cross-member checks difficult to do in *clap*. This returns
    /// a _Config_ instance whose members provide the desired values, or an error if the values are not allowed.
    fn load() -> Result<Config, &'static str> {
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
            if let Err(msg) = Input::init(config.text.clone(), config.files.clone()) {
                println!("{}", msg);
                while let Err(m2) = Input::next() {  println!("{}", m2); }
            }
            let mut start: usize = 0;
            let mut count: usize = 0;
            let match_number: usize = if config.all { 0 } else { config.count as usize };
            loop {
                match walk_tree(&tree, start) {
                    Err(msg) => println!("{}", msg),
                    Ok(None) => {
                        loop {
                            match Input::next() {
                                Err(msg) => println!("{}", msg),
                                Ok(false) => {
                                    if count == 0 { println!("No match"); }
                                    return;
                                },
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
                        count += 1;
                        start = path.end();
                    }
                }
                if count == match_number { break; }
            }
        }
    }
}

