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
//! Putting "\c" at the front of a string ignores case for that match only.
//!
//! ## Alternate RE syntax
//!
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
//! Usage: regexp \[OPTIONS\] \[RE\] \[FILES\]...
//!
//! Arguments:
//!   \[RE\]        Regular expression to search for (required unless --interactive) \[default: ""\]
//!   \[FILES\]...  Files to search, file to search
//!
//! Options:
//!   -t, --text \<TEXT\>      \[default: "" \]
//!   -p, --parser \<PARSER\>  Parser to use. Will accept abbreviations. Currently supported are 'traditional' and 'alternative' [default: traditional]
//!   -i, --interactive      Start up an interactive session
//!   -T, --tree             Prints the parsed regexp tree
//!   -w, --walk             Dumps the current path (the successful path, if called on the result of walk())
//!   -d, --debug \<DEBUG\>    Prints debug information. 1 - 8 give progressively more data [default: 0]
//!   -n, --named            Prints result for all named units
//!   -a, --all              find all instances instead of just first
//!   -c, --count \<COUNT\>    number of matches to find. Overruled by --all if it appears [default: 1]
//!   -q, --quiet            just print out matched strings, no details or names
//!   -h, --help             Print help
//!   -V, --version          Print version
//!
//! #### API
//! A search has three phases. The first phase parses the regular expression to get a regular expression tree, which is the map needed to
//! search the target string. The second phase uses the tree to walk through the target string to see if there is a match. Finally, the
//! third phase takes the Path returned by the walk phase and generates the results in a more accessible form.
//!
//! A simple example of how to use it is:
//!
//! ```
//! fn search(regexp: &str, text: &str) -> Result<Option<Report>, String>{
//!     let tree = match regexp::parse_tree(regexp) {
//!         Ok(node) => node,
//!         Err(error) => { return Err(error); },
//!     };
//!     stderr::Input::init_text(text);      // sets the string to search to TEXT
//!     match regexp::walk_tree(&tree, 0) {
//!         Ok(Some((path, char_start, bytes_start))) => {
//!             return Ok(Some(regexp::Report::new(&path, char_start, bytes_start).display(0)))
//!         },
//!         Ok(None) => return Ok(None),
//!         Err(error) => Err(error),
//!     }
//! }
//!
//!```
//!
//!
//! THere are 3 functions to choose from to initialize the buffer:
//! Input::init_text() to search a text string, Input::init_files() to
//! search the contents of a list of files, and input::init_stdin() to
//! search a string from STDIN. The START argument to walk_tree()
//! gives the position to start the search from. This is needed to
//! find all instances, the regexp library only finds a single
//! instance.
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

mod interactive;
pub mod regexp;
mod tests;

use crate::interactive::Interactive;
use crate::regexp::*;

/// Main function to run regexp as a function. It is called by
/// > cargo run [-t] [-i] [-d LEVEL] [-a LENGTH] \[REGEXP\] \[-t TARGET | FILES...\]
/// where:
///  - **REGEXP**: a regular expression. This is always required excep if _-i_ is given
///  - **TARGET**: a string to search with *REGEXP*. It is required unless _-i_ or _-t_ are given
///  - **FILES**: a list of files to search. If empty or the first file is "-" STDIN is used
///  - **-t** (**--tree**): just parse the regexp to make the regexp tree and print it out in a user friendly format
///  - **-i** (**--interactive**): run an interactive session. This lets the user enter regexps and targets and run them to see the
/// results, as well as viewing the details of the tree parse or tree walk phases.
///  - **-d LEVEL** (**--debug LEVEL**): set the debug level to LEVEL. The default level is 0, good values to try are 1, 2, or 3.
///  - **-a** (**--all**): Finds all instances in the input string. By default only the first is found.
///  - **-c COUNT** (**--count COUNT**): finds the first COUNT occurences and exits. The default is 1, and this is overruled by the **-a** switch
pub fn main() {
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(msg) => {
            eprintln!("{}", msg);
            return;
        }
    };

    if config.interactive {
        return Interactive::new(config).run();
    }
    set_trace(config.debug as usize);
    // execution starts
    match regexp(&config) {
        Err(msg) => eprintln!("{}", msg),
        Ok(count) => {
            if !config.quiet {
                let file_count = Input::file_count();
                if file_count > 0 {
                    eprintln!("Found {} instances in {} files", count, file_count);
                } else {
                    eprintln!("Found {} instances", count);
                }
            }
        }
    }
}
