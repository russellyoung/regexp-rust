#![allow(dead_code)]
mod regexp;

//use crate::regexp;
//use std::env;
use clap::Parser;

const INTERACTIVE_DEFAULT: bool = false;
const PRINTTREE_DEFAULT: bool = false;

#[derive(Parser, Debug)]
#[command(author, version, about, verbatim_doc_comment)]
///
/// This is a regular expression search utility written in rust
/// with 2 lines
pub struct Input {
    #[clap()]
    pub re: String,
    #[clap(default_value_t = String::from(""))]
    pub text: String,
    #[clap(short, long, default_value_t = INTERACTIVE_DEFAULT)]
    pub interactive: bool,
    #[clap(short, long, default_value_t = PRINTTREE_DEFAULT)]
    pub tree: bool,
}

fn check_config(config: &Input) -> Option<&str> {
    if config.text == "" && !config.tree {
        Some("Either -t (show parse tree) or TEXT is required")
    } else {None}
}

fn main() {
//    regexp::xxxx();
    let config = Input::parse();
    let error = check_config(&config);
    if error.is_some() {
        println!("{}", error.unwrap());
    } else {
        println!("{:#?}", config);
    }
    
}
