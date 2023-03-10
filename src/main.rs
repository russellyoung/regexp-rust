#![allow(dead_code)]
mod regexp;

//use crate::regexp;
//use std::env;
use clap::Parser;

const INTERACTIVE_DEFAULT: bool = false;
const PRINTTREE_DEFAULT: bool = false;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Input {
    #[clap()]
    pub re: String,
    #[clap()]
    pub text: String,
    #[clap(short, long, default_value_t = INTERACTIVE_DEFAULT)]
    pub interactive: bool,
    #[clap(short, long, default_value_t = PRINTTREE_DEFAULT)]
    pub tree: bool,
}

fn main() {
    let config = Input::parse();
    println!("{:#?}", config);
    
}
