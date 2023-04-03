#![allow(dead_code)]
mod regexp;

//use crate::regexp;
//use std::env;
use crate::regexp::{Report, Node};

use clap::{Parser, value_parser};               // Command Line Argument Processing
use std::io;
use std::io::prelude::*;
use std::collections::HashMap;

// interactive mode (TODO)
const INTERACTIVE_DEFAULT: bool = false;
// print RE parse tree
const PRINTTREE_DEFAULT: bool = false;
// print debugging messages
const DEBUG_DEFAULT: u32 = 0;
const ABBREV_DEFAULT: u32 = 5;

const TAB_SIZE:isize = 4;                     // indent in Debug display

// Used for debugging: the function trace(Path) either is a no-op or prints the given path, depending on the command line args
//static mut trace = |x| { println!("{:#?}", x) };
// TODO: make this a macro
static mut TRACE_LEVEL: u32 = 0;
fn set_trace(level: u32) { unsafe { TRACE_LEVEL = level }}
pub fn trace(level: u32) -> bool { unsafe { level <= TRACE_LEVEL }}

static mut TRACE_INDENT:isize = 0;
// when assigning trace levels to print statements make sure lines that change the indent level have the same trace level
pub fn trace_change_indent(delta: isize) { unsafe { TRACE_INDENT += delta; } }
pub fn trace_set_indent(size: isize) { unsafe { TRACE_INDENT = size; } }
pub fn trace_indent() -> String { unsafe { pad(TRACE_INDENT) }}

// helper function to format debug
fn pad(x: isize) -> String {
    let pad = { if x < 0 {0} else {(TAB_SIZE*x) as usize}};
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

fn main() {
    let config = match Config::get() {
        Ok(cfg) => cfg,
        Err(msg) => {
            println!("{}", msg);
            return;
        }
    };
    if config.interactive {
        return Interactive::new(config).run();
    }
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
        match regexp::walk_tree(&tree, &config.text) {
            Ok(Some((path, char_start, bytes_start))) => Report::new(&path, char_start, bytes_start).display(0),
            Ok(None) => println!("No match"),
            Err(error) => println!("{}", error)
        }
    }
}

const PROMPT: &str = "> ";

struct Interactive {
    res: Vec<String>,
    texts: Vec<String>,
    tree: Node,
    cmd_parse_tree: Node,
    prompt_str: String,
    abbrev: u32,
}

//const CMD_PARSE_RE:&str = r"^ *\(?<cmd>[rth?][a-z]*\) *\(?<body>\(?<subcmd>[a-z]+\|[0-9]*\) *\(?<tail>.*\)\)?";
const CMD_PARSE_RE:&str = r"^ *\(?<all>\(?\(?<cmd>[rth?][a-z]*\)[^a-z]\|$\) *\(?<body>\(?\(?<subcmd>[a-z]+\)[^a-z]\|$\)\|\(?<num>[0-9]*\)[^0-9]\|$ *\(?<tail>.*\)\)?\)";

fn get_var<'a>(vars: &HashMap<&'a str, Vec<(&'a String, (usize, usize), (usize, usize))>>, name: &'a str) -> &'a str {
    if let Some(var) = vars.get(name) { var[0].0.as_str() } else { "" }
}
fn looks_like_re(string: &str) -> bool { string.contains('\\') || string.contains('*') || string.contains('+')}

impl Interactive {
    fn new(config: Config) -> Interactive {
        let mut res = Vec::<String>::new();
        if !config.re.is_empty() { res.push(config.re.to_string()); }
        let mut texts = Vec::<String>::new();
        if !config.text.is_empty() { texts.push(config.text.to_string()); }
        Interactive { res,
                      texts,
                      tree: Node::None,
                      cmd_parse_tree: regexp::parse_tree(&CMD_PARSE_RE).unwrap(),
                      prompt_str: PROMPT.to_string(),
                      abbrev: config.abbrev,
        }
    }
    
    fn prompt(&mut self) {
        if self.res.is_empty() { print!("(RE) {} ", self.prompt_str); }
        else if self.texts.is_empty() { print!("(TEXT) {} ", self.prompt_str); }
        else { print!("{} ", self.prompt_str); }
        std::io::stdout().flush().unwrap();
    }

    fn run(&mut self) {
        let stdin = io::stdin();
        let mut buffer;
        loop {
            self.prompt();
            buffer = "".to_string();
            match stdin.read_line(&mut buffer) {
                Ok(0) => { break; },
                Ok(1) => (),
                Ok(_x) => { let  _ = buffer.pop(); self.do_command(&buffer);},
                Err(_msg) => { break;},
            }
        }
        println!("exit");
    }
    
    fn do_command(&mut self, input: &String) {
        let walk = regexp::walk_tree(&self.cmd_parse_tree, &input);
        if let Ok(Some((path, _, _))) = &walk {
            let report = Report::new(&path, 0, 0);
            let vars = report.get_named();
            //            let (cmd, subcmd, body, tail) = (vars.get("cmd").unwrap()[0].0,
//                                             vars.get("subcmd").unwrap()[0].0,
//                                             vars.get("body").unwrap()[0].0,
//                                             vars.get("tail").unwrap()[0].0);
//            println!("'{}', '{}', '{}', '{}'", cmd, subcmd, body, tail);
            self.execute_command(get_var(&vars, "cmd"),      // first word
                                 get_var(&vars, "subcmd"),   // second word if it is alphabetic
                                 get_var(&vars, "num"),      // second word if it is numeric
                                 get_var(&vars, "tail"),     // stuff after second word
                                 get_var(&vars, "body"),   // everything after the first word
                                 get_var(&vars, "all"),);    // everything
        } else {
            self.execute_command("", "", "", "", "", input);
        }
    }

    fn execute_command(&mut self, cmd: &str, subcmd: &str, num: &str, tail: &str, body: &str, all: &str) {
        println!("cmd: '{}', subcmd: '{}', num: '{}', tail: '{}', body: '{}', all: '{}'", cmd, subcmd, num, tail, body, all);
        if cmd.is_empty() {
            if looks_like_re(all) {
                println!("guessing \"{}\" is a RE", all);
                self.res.push(all.to_string());
            }
            else { println!("Unrecognized command"); }
        }
        else if "re".starts_with(cmd) {
            if !num.is_empty() {
                if let Ok(num) = num.parse::<usize>() {
                    if num >= self.res.len() { println!("Number too large, no such RE"); }
                    else if num < self.res.len() { 
                        let re = self.res.remove(self.res.len() - 1 - num);
                        println!("Using RE \"{}\"", re);
                        self.res.push(re);
                    } else {}
                }
            } else if !subcmd.is_empty() {
                if "pop".starts_with(subcmd) {
                    let _ = self.res.pop();
                    if self.res.is_empty() { println!("No current RE"); }
                    else { println!("current RE is \"{}\"", self.res[self.res.len() - 1]); }
                } else if "history".starts_with(subcmd) {
                    let len = self.res.len();
                    if len == 0 { println!("No saved REs"); }
                    else { for i in 0..len { println!("  {}: \"{}\"", i, self.res[len - i - 1]); } }
                } else if "set".starts_with(subcmd) { self.res.push(tail.to_string()); }
                else if looks_like_re(body) {
                    println!("guessing \"{}\" is a RE", body);
                    self.res.push(body.to_string());
                } else { println!("Unrecognized subcommand"); }
            } else if looks_like_re(body) {
                println!("guessing \"{}\" is a RE", body);
                self.res.push(body.to_string());
            } else if body.is_empty() {
                if self.res.is_empty() { println!("No current RE"); }
                else { println!("current RE is \"{}\"", self.res[self.res.len() - 1]); }
            } else { println!("Unrecognized subcommand"); }
        } else if "text".starts_with(cmd) {
            if !num.is_empty() {
                if let Ok(num) = num.parse::<usize>() {
                    if num >= self.texts.len() { println!("Number too large, no such text"); }
                    else if num < self.texts.len() { 
                        let text = self.texts.remove(self.texts.len() - 1 - num);
                        println!("Using text \"{}\"", text);
                        self.texts.push(text.to_string());
                    } else {}
                }
            } else if !subcmd.is_empty() {
                if "pop".starts_with(subcmd) {
                    let _ = self.texts.pop();
                    if self.texts.is_empty() { println!("No current RE"); }
                    else { println!("current RE is \"{}\"", self.texts[self.texts.len() - 1]); }
                } else if "set".starts_with(subcmd) { self.texts.push(tail.to_string()); }
                else if "history".starts_with(subcmd) {
                    let len = self.texts.len();
                    if len == 0 { println!("No saved texts"); }
                    else { for i in 0..len { println!("  {}: \"{}\"", i, self.texts[len - i - 1]); } }
                } else if !body.is_empty() { self.texts.push(body.to_string()); }
                else if self.texts.is_empty() { println!("No texts saved"); }
                else { println!("current text is \"{}\"", self.texts[self.texts.len() - 1]); }
            }
        } else if "help".starts_with(cmd) {
            println!("help text");
        }
    }
}

fn yorn(prompt: &str, dflt: Option<bool>) -> bool {
    let p = match dflt {
        None => "y[es] or n[o]",
        Some(true) => "[yes] or n[o]",
        Some(false) => "y[es] or [no]",
    };
    let stdin = io::stdin();
    let mut buffer = String::new();
    loop {
        print!("{} ({}): ", prompt, p);
        std::io::stdout().flush().unwrap();
        if let Ok(count) = stdin.read_line(&mut buffer) {
            if count < 2 { break; }
            let _ = buffer.pop();
            println!("{:#?}", buffer);
            if "yes".starts_with(&buffer) { return true; }
            if "no".starts_with(&buffer) { return false; }
            println!("???{:#?}", buffer);
        } else { println!("error reading input"); }
    }
    if let Some(d) = dflt { d } else {yorn(prompt, dflt)}
}
