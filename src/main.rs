#![allow(dead_code)]
mod regexp;

//use crate::regexp;
//use std::env;
use clap::Parser;

// interactive mode (TODO)
const INTERACTIVE_DEFAULT: bool = false;
// print RE parse tree
const PRINTTREE_DEFAULT: bool = false;
// print the current walk path as each step is taken
const WALKTRACE_DEFAULT: bool = false;

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
    #[clap(short, long, default_value_t = WALKTRACE_DEFAULT)]
    pub walk: bool,
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
    // execution starts
    let tree = match regexp::parse_tree(&config.re) {
        Ok(node) => node,
        Err(msg) => {
            println!("Error parsing regexp: {}", msg);
            return;
        },
    };
    if config.tree {
        println!("{:?}", tree);
    }
    if !config.text.is_empty() {
        regexp::walk::set_walk_trace(config.walk);
        match regexp::walk_tree(&tree, &config.text) {
            Some(result) => println!("{:?}", result.display()),
            None => println!("No match"),
        }
    }
}
