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
use core::fmt::{Debug,};

const PROMPT: &str = "> ";

struct RegExp {
    re: String,
    alt_parser: bool
}

impl Debug for RegExp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} regular expression \"{}\"", if self.alt_parser {"alternative" } else { "traditional" }, self.re)
    }
}

impl RegExp {
    fn guess_type(maybe_re: &str) -> Option<RegExp> {
        let (prompt, dflt) = if maybe_re.contains("and(") || maybe_re.contains("or(") || maybe_re.contains("get(") {
            ("This looks like an alternative RE type. It is t[raditional], [alternative], c[ancel]", 1)
        } else if maybe_re.contains('\\') || maybe_re.contains('*') || maybe_re.contains('+') {
            ("This looks like a traditional RE type: [traditional], a[lternative], c[ancel]", 0)
        } else {
            ("This doesn't look like a regular expression. It is: [traditional], [alternative], c[ancel]", 2)
        };
        match get_response(prompt, vec!["traditional", "alternative", "cancel"], dflt) {
            "alternative" => Some(RegExp {re: maybe_re.to_string(), alt_parser: true}),
            "traditional" => Some(RegExp {re: maybe_re.to_string(), alt_parser: false}),
            _ => None
        }
    }
}
    /// The structure used to run an interactive session
pub(crate) struct Interactive {
    /// the list of regular expressions, last one is the current value
    res: Vec<RegExp>,
    /// the list of target strings, last one is the current value
    texts: Vec<String>,
    // Interesting: I stored cmd_parse_tree here to avoid having to recompute it each time, and found
    // it caused a borrow violation. Moving it out and passing it as an extra parameter to where it is used
    // removed the problem. Lesson learned.
    // the tree used to parse the user commands, parsed from CMD_PARSE_RE, to save it from having to be reparsed every time
    // cmd_parse_tree: Node,
}

/// a RE used to parse the command line each time it is entered.
// It gets named matches:
//  - cmd: the first word
//  - subcmd: the second word, if alphabetic
//  - num: the second word, if numeric
//  - body: everything after the first word
//  - tail:  everything after the secondword
//  - all: the whole command, trimmed

//const CMD_PARSE_RE:&str = r"^ *\(?<all>\(?\(?<cmd>[rtslfwh?][a-z]*\)[^a-z]\|$\) *\(?<body>\(?\(?<subcmd>[a-z]+\)[^a-z]\|$\)\|\(?<num>[0-9]*\)\|$ *\(?<tail>.*\)\)?\)";
const CMD_PARSE_ALT_RE: &str = r"^and('\w*' '[^\w]'+<words>)+";

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
 - walk [level]: displays the progress as the tree is walked in stage 2 of the search. LEVEL defaults to 2
                 to provide moderate information, use 1 for less information, 3 for more
 - help:         displays this help
 - ?:            displays this help
";

const COMMANDS: [&str; 7] = ["re", "text", "search", "tree", "walk", "help", "?"];
/// parse the command from the possily abbreviated version passed in
fn get_command(cmd: &str) -> &str {
    if cmd.is_empty() { "" } else {
        match COMMANDS.iter().filter(|x| x.starts_with(cmd)).collect::<Vec<&&str>>()[..] {
            [] => "unrecognized",
            [x] => x,
            _ => "ambiguous"
        }
    }
}

fn input_substring<'a> (words: &Vec<&'a Report>, from: usize, to: usize) -> &'a str {
    let len = words.len();
    if from >= len { "" }
    else {
        let r0 = words[from]; 
        let r1 = if to < len { words[to] } else { words[len - 1] }; 
        &r0.full_string()[r0.byte_pos().0..r1.byte_pos().1]
    }
}

fn int_arg(words: &Vec<&Report>, arg_num: usize, dflt: usize) -> Option <usize> {
    let arg = input_substring(words, arg_num, arg_num);
    if arg.is_empty() { Some(dflt) }
    else if let Ok(num) = arg.parse::<usize>() { Some(num) }
    else { None }
}

impl Interactive {
    /// constructor for the session object
    pub(crate) fn new(config: Config) -> Interactive {
        let mut res = Vec::<RegExp>::new();
        if !config.re.is_empty() { res.push(RegExp {re: config.re.to_string(), alt_parser: config.alt_parser()}); }
        let mut texts = Vec::<String>::new();
        if !config.text.is_empty() { texts.push(config.text); };
        Interactive { res,
                      texts,
//                      cmd_parse_tree: parse_tree(CMD_PARSE_ALT_RE, true).unwrap()
        }
    }

    /// gets the current RE, or None
    fn re(&self) -> Option<&RegExp> { self.res.last() }
    
    /// gets the current search text, or None
    fn text(&self) -> Option<&String> {
        self.texts.last()
    }

    /// starts up the interactive session
    pub(crate) fn run(&mut self) {
        let stdin = io::stdin();
        let mut buffer;
        let cmd_parse_tree = parse_tree(CMD_PARSE_ALT_RE, true).unwrap();
        loop {
            buffer = "".to_string();
            self.prompt();
            match stdin.read_line(&mut buffer) {
                Ok(0) => { break; },
                Ok(1) => (),
                Ok(_x) => {
                    let  _ = buffer.pop();    // pop off trailing CR
                    if !self.do_command(&buffer, &cmd_parse_tree) {break; }
                },
                Err(_msg) => { break;},
            }
        }
        println!("exit");
    }

    /// prints out the session prompt - maybe in the future it will want to display some information here 
    fn prompt(&mut self) {
        print!("> ");
        std::io::stdout().flush().unwrap();
    }

    /// parses the entered string to get a command, and call **execute_command()** to do it. Return *false* to exit.
    fn do_command(&mut self, input: &str, cmd_parse_tree: &Node) -> bool{
        match walk_tree(cmd_parse_tree, input) {
            Ok(Some(path)) => {
                let report = Report::new(&path);
                let words = report.get_by_name("words");
                self.execute_command(&words)
            },
            Ok(None) => true,
            Err(msg) => { println!("{}", msg); true }
        }
    }

    /// execute the user commands
    fn execute_command(&mut self, words: &Vec<&Report>) -> bool {
        match get_command(words[0].string()) {
            "re" => self.do_re(words),
            "text" => self.do_text(words),
            "search" => self.do_search(words),
            "help" | "?" => println!("{}", HELP_TEXT),
            "quit" => { return false; },
            "exit" => { return get_response("Really exit?", vec!["[yes]", "n[o]"], 0) == "no"; },
            "tree" => self.do_tree(words),
            "" => (),
            "unrecognized" => println!("unrecognized command"),
            "ambiguous" => println!("ambiguous command"),
            _ => (),
        }
        true
    }

    /// execute a *re* command
    fn do_re(&mut self, words: &Vec<&Report>) {
        let arg1 = input_substring(words, 1, 1);
        if arg1.is_empty() {
            if let Some(re) = self.re() { println!("current RE: \"{:?}\"", re); }
            else { println!("No REs stored"); }
        } else if let Ok(num) = arg1.parse::<usize>() {
            if num >= self.res.len() { println!("There are only {} REs stored", self.res.len()); }
            else {
                let re = self.res.remove(self.res.len() - 1 - num);
                println!("Using {:?}", re);
                self.res.push(re);
            }
        } else if "pop".starts_with(arg1) {
            let _ = self.res.pop();
            if let Some(re) = self.re() {println!("current RE is {:?}", re); }
            else { println!("No current RE"); }
        } else if "history".starts_with(arg1) || "list".starts_with(arg1) {
            let len = self.res.len();
            if len == 0 { println!("No saved REs"); }
            else { for i in 0..len { println!("  {}: {:?}", i, self.res[len - i - 1]); } }
        } else if "traditional".starts_with(arg1) {
            if words.len() == 2 {println!("'re traditional' requires regular expression");}
            else { self.res.push(RegExp {re: input_substring(words, 2, 1000).to_string(), alt_parser: false}); }
        } else if "alternative".starts_with(arg1) {
            if words.len() == 2 {println!("'re alternative' requires regular expression");}
            else { self.res.push(RegExp {re: input_substring(words, 2, 1000).to_string(), alt_parser: true}); }
        } else if let Some(re) = RegExp::guess_type(input_substring(words, 1, 1000)) {
            self.res.push(re);
        } else { println!("Unrecognized re subcommand"); }
    }

    /// execute a *text* command
    fn do_text(&mut self, words:&Vec<&Report>) {
        let arg1 = input_substring(words, 1, 1);
        if arg1.is_empty() { 
            if let Some(text) = self.text() {println!("current text: \"{:?}\"", text); }
            else { println!("No texts stored"); }
        } else if let Ok(num) = arg1.parse::<usize>() {
            if num >= self.texts.len() { println!("Number too large, no such text"); }
            else {
                let text = self.texts.remove(self.texts.len() - 1 - num);
                println!("Using {:?}", text);
                self.texts.push(text);
            }
        } else if "pop".starts_with(arg1) {
            let _ = self.texts.pop();
            if let Some(text) = self.text() {println!("current text is \"{}\"", text); }
            else { println!("No current text"); }
        } else if "set".starts_with(arg1) {
            self.texts.push(input_substring(words, 2, 1000).to_string());
        } else if "history".starts_with(arg1) {
            let len = self.texts.len();
            if len == 0 { println!("No saved texts"); }
            else { for i in 0..len { println!("  {}: \"{}\"", i, self.texts[len - i - 1]); } }
        } else { self.texts.push(input_substring(words, 1, 1000).to_string()); }
    }

    fn do_tree(&self, words: &Vec<&Report>) {
        let trace_level = if let Some(num) = int_arg(words, 1, 0) {
            num
        } else {
            println!("'tree' takes an optional integer argument");
            return;
        };
        if let Some(re) = self.re() {
            set_trace(trace_level);
            match parse_tree(&re.re, re.alt_parser) {
                Ok(node) => { println!("--- Parse tree:"); node.desc(0); }
                Err(error) => println!("Error parsing tree: {}", error),
            }
            set_trace(0);
        } else { println!("No current RE, first enter one"); }
    }
    
    fn do_search(&self, words: &Vec<&Report>) {
        let trace_level = if let Some(num) = int_arg(words, 1, 0) {
            num
        } else {
            println!("'search' takes an optional integer argument");
            return;
        };
        match (self.re(), self.text()) {
            (None, Some(_)) => println!("No current regular expression"),
            (Some(_), None) => println!("No current text"),
            (None, None) => println!("No regular expression or text, add some and try again"),
            (Some(re), Some(text)) => {
                match parse_tree(re.re.as_str(), re.alt_parser) {
                    Err(err) => println!("Error parsing RE: {}", err.msg),
                    Ok(node) => {
                        set_trace(trace_level);
                        match walk_tree(&node, text) {
                            Err(msg) => println!("Error: {}", msg),
                            Ok(None) => println!("No match"),
                            Ok(Some(path)) => println!("{:?}", Report::new(&path).display(0)),
                        }
                        set_trace(0);
                    }
                }
            }
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

fn get_response<'a>(prompt: &'a str, choices: Vec<&'a str>, dflt: usize) -> &'a str {
    let mut buffer = String::new();
    let stdin = io::stdin();
    loop {
        print!("{}: ", prompt);
        std::io::stdout().flush().unwrap();
        if let Ok(count) = stdin.read_line(&mut buffer) {
            if count < 2 { break; }   // take default
            let _ = buffer.pop();
            let mut candidate: Option<&str> = None;
            for c in choices.iter() {
                if c.starts_with(buffer.as_str()) {
                    if candidate.is_none() {
                        candidate = Some(c);
                    } else {println!("ambiguous response"); }
                }
            }
            if let Some(cmd) = candidate { return cmd; }
            else { println!("Unrecognized command"); }
        } else { println!("error reading input"); }
    }
    if dflt < choices.len() { return choices[dflt]; }
    println!("no default response");
    get_response(prompt, choices, dflt)
}


    
