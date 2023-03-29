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
const TAB_SIZE:usize = 2;                     // indent in Debug display

// Used for debugging: the function trace(Path) either is a no-op or prints the given path, depending on the command line args
//static mut trace = |x| { println!("{:#?}", x) };
// TODO: make this a macro
static mut TRACE_LEVEL: u32 = 0;
fn set_trace(level: u32) { unsafe { TRACE_LEVEL = level }}
pub fn trace(level: u32) -> bool { unsafe { level <= TRACE_LEVEL }}

static mut TRACE_INDENT:isize = 0;
pub fn trace_change_indent(delta: isize) { unsafe {TRACE_INDENT += delta; }}
pub fn trace_indent() -> String { unsafe { pad(TRACE_INDENT as usize) }}

// helper function to format debug
fn pad(x: usize) -> String {
    let pad = TAB_SIZE*x;
    format!("{:pad$}", "")
}

// trace 2: print walk enter, walk exit
// trace 4: print step enter, step exit
// trace 6: print backoff stuff
#[derive(Parser, Debug)]
#[command(author, version, about, verbatim_doc_comment)]
///
/// This is a regular expression search utility written in rust
/// with 2 lines
pub struct Config {
    #[clap()]
    pub re: String,
    #[clap(default_value_t = String::from(""))]
    pub text: String,
    #[clap(short, long, default_value_t = INTERACTIVE_DEFAULT)]
    pub interactive: bool,
    #[clap(short, long, default_value_t = PRINTTREE_DEFAULT)]
    pub tree: bool,
    #[clap(short, long, default_value_t = DEBUG_DEFAULT, value_parser=value_parser!(u32).range(0..40))]
    pub debug: u32, 
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
    // execution starts
    let tree = match regexp::parse_tree(&config.re) {
        Ok(node) => node,
        Err(msg) => {
            println!("Error parsing regexp: {}", msg);
            return;
        },
    };
    if config.tree {
        println!("--- Parse tree:\n{:?}", tree);
    }
    if !config.text.is_empty() {
        let result = regexp::walk_tree(&tree, &config.text);
        match result {
            Some(path) => { if let Some(report) = path.report() { report.display(0); } else {println!("{}", path.matched_string());} },
            None => println!("No match"),
        }
    }
}
