//! ## Interactive Regular Expression Search
//! This module provides an interactive application that allows a user to enter several regular expression and text strings,
//! and then perform searches. It can be useful when trying to write a complicated regular expression to try it out as you go.
//! 
//! In addition, it provides features to dump out the regular expression tree and trace the walk phase as it looks for a match.
//! It can be run by adding the **-i** switch to the program when starting it. 
//! 
//! The interactive program holds multiple expressions and text strings, and can operate on the top-level one of each. Operations
//! include performing a search, printing out a search tree, printing out **Path**s as they are being walked. To get complete
//! directions on how to use it use the command 'help' or '?' after starting it up.
//! 
//! While this can help in writing complex regular expressions or in understanding how the parser and walker work, it was mainly 
//! as an exercise in Rust.

use crate::regexp::*;
use crate::set_trace;
use crate::Config;
use std::io;
use std::io::Write;    
use std::collections::HashMap;

const PROMPT: &str = "> ";

/// The structure used to run an interactive session
pub(crate) struct Interactive {
    /// the list of regular expressions, last one is the current value
    res: Vec<String>,
    /// the list of target strings, last one is the current value
    texts: Vec<String>,
//    tree: Node,
    /// the tree used to parse the user commands, parsed from CMD_PARSE_RE, to save it from having to be reparsed every time
    cmd_parse_tree: Node,
    /// the prompt string to use: can be reset (originally it was to reflect the state, now it is constant and this can be removed)
    prompt_str: String,
}

/// a RE used to parse the command line each time it is entered.
// It gets named matches:
//  - cmd: the first word
//  - subcmd: the second word, if alphabetic
//  - num: the second word, if numeric
//  - body: everything after the first word
//  - tail:  everything after the secondword
//  - all: the whole command, trimmed

const CMD_PARSE_RE:&str = r"^ *\(?<all>\(?\(?<cmd>[rtsfwh?][a-z]*\)[^a-z]\|$\) *\(?<body>\(?\(?<subcmd>[a-z]+\)[^a-z]\|$\)\|\(?<num>[0-9]*\)\|$ *\(?<tail>.*\)\)?\)";

/// help text to display
const HELP_TEXT: &str = r"
This is an interactive interface to the regexp search engine. The program keeps stacks of
regular expressions and search texts and uses them to run searches. Besides simple searching
the program will print out the parsed search tree and also details of the walk over the target
string.

Commands are in general of the form CMD [SUBCMD [DATA]], though it will try to guess the 
meaning of ambiguous commands. The commands and subcommands can be abbreviated with the 
first couple unique letters.

The commands are:
 - re:           display the current active regular expression
 - re [set] RE:  sets a new regular expression to be the current one. The 
                 SET keyword is optional, if not given the program usually will guess the text is
                 intended as a regular expression. Only in the rare case when the text starts with a 
                 keyword is the SET subcommand required.
 - re history:   lists the most recent regular expressions
 - re NUMBER:    sets the NUMBERth item on the history list to be the current regular expression
 - re pop:       pops off (deletes) the current re from the list
 - text:         displays the current active text string
 - text TEXT:    sets new search text
 - text history: lists the most recent regular expressions
 - text NUMBER:  sets the NUMBERth item on the history list to be the current regular expression
 - text pop:     pops off (deletes) the current re from the list
 - search:       performs a RE search using the current RE and the current text
 - tree:         displays the parse tree created in the first stage of the RE search using the current RE
 - walk:         displays the progress as the tree is walked in stage 2 of the search
 - help:         displays this help
 - ?:            displays this help
";

/// gets the matching string for a single named variable from the search results. This is used to parse the used input
fn get_var<'a>(vars: &HashMap<&'a str, Vec<&'a Report>>, name: &'a str) -> &'a str {
    if let Some(var) = vars.get(name) { var[0].found.as_str() } else { "" }
}

/// parse the command from the possily abbreviated version passed in
fn get_command(cmd: &str) -> &str {
    if cmd.is_empty() { "" }
    else if "re".starts_with(cmd) { "re" }
    else if "text".starts_with(cmd) { "text" }
    else if "search".starts_with(cmd) { "search" }
    else if "tree".starts_with(cmd) { "tree" }
    else if "walk".starts_with(cmd) { "walk" }
    else if "help".starts_with(cmd) || "?" == cmd { "help" }
    else { "unrecognized"}
}
    
impl Interactive {
    /// constructor for the session object
    pub(crate) fn new(config: Config) -> Interactive {
        let mut res = Vec::<String>::new();
        if !config.re.is_empty() { res.push(config.re.to_string()); }
        let mut texts = Vec::<String>::new();
        if !config.text.is_empty() { texts.push(config.text); }
        Interactive { res,
                      texts,
//                      tree: Node::None,
                      cmd_parse_tree: parse_tree(CMD_PARSE_RE).unwrap(),
                      prompt_str: PROMPT.to_string(),
        }
    }

    /// gets the current RE, or None
    fn re(&self) -> Option<&str> { if self.res.is_empty() { None } else { Some(self.res[self.res.len() - 1].as_str()) }}
    /// gets the current search text, or None
    fn text(&self) -> Option<&str> { if self.texts.is_empty() { None } else { Some(self.texts[self.texts.len() - 1].as_str()) }}

    /// starts up the interactive session
    pub(crate) fn run(&mut self) {
        let stdin = io::stdin();
        let mut buffer;
        loop {
            self.prompt();
            buffer = "".to_string();
            match stdin.read_line(&mut buffer) {
                Ok(0) => { break; },
                Ok(1) => (),
                Ok(_x) => {
                    let  _ = buffer.pop();    // pop off trailing CR
                    if !self.find_command(&buffer) {break; }
                },
                Err(_msg) => { break;},
            }
        }
        println!("exit");
    }

    /// tries guessing if a string could be a RE, if it is not sure it asks with **yorn()** (*yes-or-no()*)
    fn guess_re(&mut self, maybe_re: &str) -> bool {
        if (maybe_re.contains('\\') || maybe_re.contains('*') || maybe_re.contains('+'))
            && yorn(&format!("\"{}\" looks like a RE. Is it?", maybe_re), Some(true)) {
                self.res.push(maybe_re.to_string());
                true
            }
        else { false }
    }

    /// prints out the session prompt
    fn prompt(&mut self) {
        print!("{} ", self.prompt_str);
        std::io::stdout().flush().unwrap();
    }

    /// parses the entered string to get a command, and call **execute_command()** to do it. Return *false* to exit.
    fn find_command(&mut self, input: &str) -> bool{
        let walk = walk_tree(&self.cmd_parse_tree, input);
        if let Ok(Some((path, _, _))) = &walk {
            let report = Report::new(path, 0, 0);
            let vars = report.get_named();
            self.execute_command(get_var(&vars, "cmd"),      // first word
                                 get_var(&vars, "subcmd"),   // second word if it is alphabetic
                                 get_var(&vars, "num"),      // second word if it is numeric
                                 get_var(&vars, "tail"),     // stuff after second word
                                 get_var(&vars, "body"),     // everything after the first word
                                 get_var(&vars, "all"),)     // everything, trimmed
        } else {
            self.execute_command("", "", "", "", "", input)
        }
    }

    /// execute the user commands
    fn execute_command(&mut self, cmd: &str, subcmd: &str, num: &str, tail: &str, body: &str, all: &str) -> bool{
        //println!("cmd: '{}', subcmd: '{}', num: '{}', tail: '{}', body: '{}', all: '{}'", cmd, subcmd, num, tail, body, all);
        match get_command(cmd) {
            "" => { if !all.is_empty() && !self.guess_re(all) { println!("Unrecognized command"); }},
            "re" => self.do_re(subcmd, num, tail, body),
            "text" => self.do_text(subcmd, num, tail, body),
            "search" => self.do_search(subcmd),
            "help" | "?" => println!("{}", HELP_TEXT),
            "walk" => self.do_walk(),
            "quit" => { return false; },
            "exit" => { if yorn("Really exit?", Some(true)) { return false; } }
            "tree" => {
                if let Some(re) = self.re() {
                    match parse_tree(re) {
                        Ok(node) => println!("--- Parse tree:\n{:?}", node),
                        Err(error) => println!("Error parsing tree: {}", error),
                    }
                } else { println!("No current RE, first enter one"); }
            },
            "unrecognized" => println!("unrecognized command"),
            _ => (),
        }
        true
    }

    /// execute a *re* command
    fn do_re(&mut self, subcmd: &str, num: &str, tail: &str, body: &str, ) {
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
                if let Some(re) = self.re() {println!("current RE is \"{}\"", re); }
                else { println!("No current RE"); }
            } else if "history".starts_with(subcmd) {
                let len = self.res.len();
                if len == 0 { println!("No saved REs"); }
                else { for i in 0..len { println!("  {}: \"{}\"", i, self.res[len - i - 1]); } }
            } else if "set".starts_with(subcmd) { self.res.push(tail.to_string()); }
            else if !self.guess_re(body) { println!("Unrecognized subcommand"); }
        } else if !self.guess_re(body) {
            if body.is_empty() {
                if self.res.is_empty() { println!("No current RE"); }
                else { println!("current RE is \"{}\"", self.res[self.res.len() - 1]); }
            } else { println!("Unrecognized subcommand"); }
        }
    }

    /// execute a *text* command
    fn do_text(&mut self, subcmd: &str, num: &str, tail: &str, body: &str) {
        if !num.is_empty() {
            if let Ok(num) = num.parse::<usize>() {
                if num >= self.texts.len() { println!("Number too large, no such text"); }
                else if num < self.texts.len() { 
                    let text = self.texts.remove(self.texts.len() - 1 - num);
                    println!("Using text \"{}\"", text);
                    self.texts.push(text);
                } else {}
            }
        } else if !subcmd.is_empty() {
            if "pop".starts_with(subcmd) {
                let _ = self.texts.pop();
                if let Some(text) = self.text() {println!("current text is \"{}\"", text); }
                else { println!("No current text"); }
            } else if "set".starts_with(subcmd) { self.texts.push(tail.to_string()); }
            else if "history".starts_with(subcmd) {
                let len = self.texts.len();
                if len == 0 { println!("No saved texts"); }
                else { for i in 0..len { println!("  {}: \"{}\"", i, self.texts[len - i - 1]); } }
            } else if !body.is_empty() { self.texts.push(body.to_string()); }
            else if self.texts.is_empty() { println!("No texts saved"); }
            else { println!("current text is \"{}\"", self.texts[self.texts.len() - 1]); }
        } else if body.is_empty() {
            if self.texts.is_empty() { println!("No current text"); }
            else { println!("current text is \"{}\"", self.texts[self.texts.len() - 1]); }
        } else { self.texts.push(body.to_string()); }
    }

    fn do_search(&self, subcmd: &str) {
        let re = {if let Some(r) = self.re() { r } else { println!("No current RE"); return;}};
        let text = {if let Some(t) = self.text() { t } else { println!("No current text"); return; }};
        println!("Searching for \"{}\" in \"{}\"", re, text);
        match parse_tree(re) {
            Ok(node) => {
                match walk_tree(&node, text) {
                    Ok(Some((path, char_start, bytes_start))) => {
                        let report = Report::new(&path, char_start, bytes_start);
                        if subcmd.is_empty() { println!("{:?}", report.display(0)); }
                        else {
                            let matches = report.get_by_name(subcmd);
                            if matches.is_empty() { println!("No named matches for \"{}\"", subcmd); }
                            else {
                                for i in 0..matches.len() { println!("  {}) \"{}\", position {}", i, matches[i].found, matches[i].pos.0); }
                            }
                        }
                    },                                    
                    Ok(None) => println!("No match"),
                    Err(error) => println!("Error in search: {}", error)
                }
            },
            Err(error) => { println!("Error parsing tree: {}", error); },
        }
    }

    fn do_walk(&self) {
        let re = {if let Some(r) = self.re() { r } else { println!("No current RE"); return; }};
        let text = {if let Some(t) = self.text() { t } else { println!("No current text"); return; }};
        match parse_tree(re) {
            Ok(node) => {
                set_trace(2);
                match walk_tree(&node, text) {
                    Ok(Some((path, char_start, bytes_start))) => println!("{:?}", Report::new(&path, char_start, bytes_start).display(0)),
                    Ok(None) => println!("No match"),
                    Err(error) => println!("Error in search: {}", error)
                }
                set_trace(0);
            },
            Err(error) => println!("Error parsing tree: {}", error),
        }
    }
}

/// get an answer to a yes-or-no question (it's an emacs thing)
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
