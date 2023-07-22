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
use crate::regexp::walk::Input;
use std::io;
use std::io::Write;    
use core::fmt::Debug;

const PROMPT: &str = "> ";

/// holds a RE, which consists of a string and instructions on what parser to use
struct RegExp {
    re: String,
    alt_parser: bool
}

impl Debug for RegExp {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}  \"{}\"", if self.alt_parser {"alternative" } else { "traditional" }, self.re)
    }
}

impl RegExp {
    /// Guesses the type of a regular expression and gets confirmation of its choice from the user
    fn guess_type(maybe_re: &String) -> Option<RegExp> {
        let (prompt, dflt) = if maybe_re.contains("and(") || maybe_re.contains("or(") || maybe_re.contains("get(") {
            ("This looks like an alternative RE type. It is t[raditional], [alternative], c[ancel]", 1)
        } else if maybe_re.contains('\\') || maybe_re.contains('*') || maybe_re.contains('+') {
            ("This looks like a traditional RE type: [traditional], a[lternative], c[ancel]", 0)
        } else {
            ("This doesn't look like a regular expression. It is: t[raditional], a[lternative], [cancel]", 2)
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

/// a RE used to parse the command line each time it is entered. It breaks the command line
/// up into words so it can be parsed. The Report objects do maintain the whitespace in the
/// original string so the actual entered regular expressions are not lost.
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
 - regexp:         display the current active regular expression
 - regexp [traditional | alternative] RE:  sets a new regular expression to be the current one. The 
                   keyword is optional, if not given the program usually will guess what the text
                   is, and ask for confirmation
 - regexp history: lists the most recent regular expressions
 - regexp list:    same as 're history'
 - regexp NUMBER:  sets the NUMBERth item on the history list to be the current regular expression
 - regular pop [n]:pops off (deletes) the nth re from the list. Defaults to 0 (the current RE)
 - text:           displays the current active text string
 - text TEXT:      sets new search text
 - text list:      same as 'text history'
 - text history:   lists the most recent regular expressions
 - text NUMBER:    sets the NUMBERth item on the history list to be the current text
 - text pop [n]:   pops off (deletes) the nth tex string from memory. Defauls to 0 (the current text string)
 - search :        performs a RE search using the current RE and the current text. 
 - search NAME1 [NAME2...]: performs a RE search using the current RE and the current text, report only on units with the given names
 - search * :      performs a RE search using the current RE and the current text, report on all named units
 - search NUMBER:  performs a RE search using the current RE and the current text setting debug level to NUMBER to examine the path.
                   This can be combined with search for name.
 - tree [NUMBER]:  displays the parse tree for the current regular expression. Optional **NUMBER** sets the trace level
                   to see how the parse is performed.
 - help:           displays this help
 - ?:              displays this help
";

/// The commands for the main loop
const COMMANDS: [&str; 7] = ["regexp", "text", "search", "tree", "walk", "help", "?"];
/// Used to check for continuation lines
const SLASH_BYTE: u8 = 92;
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
        let mut buffer = String::new();
        // used to signal continuation line
        let mut cont = false;
        let cmd_parse_tree = parse_tree(CMD_PARSE_ALT_RE, true).unwrap();
        loop {
            if !cont { buffer = "".to_string(); }
            self.prompt(cont);
            cont = false;
            match stdin.read_line(&mut buffer) {
                Ok(0) => { break; },
                Ok(1) => (),
                Ok(_x) => {
                    let len = buffer.len();
                    // handle line continuation
                    if let [SLASH_BYTE, _cr] = buffer.as_bytes()[len - 2..] {
                        cont = true;
                        buffer.remove(len - 2);
                    } else {
                        buffer.remove(len - 1);
                        if !self.do_command(&buffer, &cmd_parse_tree) {break; }
                    }
                },
                Err(_msg) => { break;},
            }
        }
        println!("exit");
    }

    /// prints out the session prompt - maybe in the future it will want to display some information here 
    fn prompt(&mut self, cont: bool) {
        print!("{}", if cont { "... " } else { "> "} );
        std::io::stdout().flush().unwrap();
    }

    /// parses the entered string to get a command, and call **execute_command()** to do it. Return *false* to exit.
    fn do_command(&mut self, input: &str, cmd_parse_tree: &Node) -> bool{
        if let Err(msg) = Input::init(input.to_string(), Vec::new()) { println!("{}", msg); return false; }
        match walk_tree(cmd_parse_tree, 0) {
            Ok(Some(path)) => {
                let report = Report::new(&path);
                let words = report.get_by_name("words");
                self.execute_command(&words)
            },
            Ok(None) => true,
            Err(msg) => { println!("{}", msg); true }
        }
    }
    
    /// executes the user commands
    fn execute_command(&mut self, words: &Vec<&Report>) -> bool {
        match get_command(&COMMANDS, &words[0].string()) {
            "regexp" => self.do_re(words),
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
    
    /// executes a **regexp** command
    fn do_re(&mut self, words: &Vec<&Report>) {
        let len = self.res.len();
        let subcmd = if words.len() > 1 { get_command(&["pop", "history", "list", "traditional", "alternative"], &words[1].string()) } else { "" };
        match subcmd {
            "" =>  {
                if let Some(re) = self.re() { println!("current RE: {:?}", re); }
                else { println!("No REs stored");
                }
            },
            "pop" => {
                if let Some(num) = int_arg(words, 2, 0) {
                    if num >= len {
                        println!("Only {} regular expression stored, value between 0 and {}", len, len - 1);
                    } else {
                        let _ = self.res.remove(len - 1 - num);
                        if let Some(re) = self.re() {println!("current RE is {:?}", re); }
                        else { println!("No stored regular expressions"); }
                    }
                } else { println!("regexp pop [number]"); }
            },
            "history" | "list" => {
                if len == 0 { println!("No saved REs"); }
                else { for i in 0..len { println!("  {}: {:?}", i, self.res[len - i - 1]); } }
            },
            "alternative" | "traditional" => {
                if words.len() == 2 {println!("'re {}' requires regular expression", subcmd);}
                else { self.res.push(RegExp {re: input_substring(words, 2, 1000), alt_parser: subcmd == "alternative"}); }
            },
            "ambiguous" => println!("ambiguous subcommand"),
            _ => {
                if let Some(num) = int_arg(words, 1, 0) {   // can't be default of 0, we know there is text in position 1
                    if num >= len { println!("There are only {} REs stored", len); }
                    else {
                        let re = self.res.remove(len - 1 - num);
                        println!("Using {:?}", re);
                        self.res.push(re);
                    }
                } else if let Some(re) = RegExp::guess_type(&input_substring(words, 1, 1000)) {
                    self.res.push(re);
                } else { println!("Unrecognized re subcommand"); }
            },
        }
    }
    
    /// executes a *text* command
    fn do_text(&mut self, words:&Vec<&Report>) {
        let subcmd = if words.len() > 1 { get_command(&["pop", "history", "list", "set"], &words[1].string()) } else { "" };
        let len = self.texts.len();
        match subcmd {
            "" =>  {
                if let Some(text) = self.text() { println!("current text: \"{:?}\"", text); }
                else { println!("No textsstored");
                }
            },
            "pop" => {
                if let Some(num) = int_arg(words, 2, 0) {
                    if num >= len {
                        println!("Only {} texts stored, value between 0 and {}", len, len - 1);
                    } else {
                        let _ = self.texts.remove(len - 1 - num);
                        if let Some(re) = self.re() {println!("current text is {:?}", re); }
                        else { println!("No stored texts"); }
                    }
                } else { println!("text pop [number]"); }
            },
            "set" => self.texts.push(input_substring(words, 2, 1000)),
            "history" | "list" => {
                if len == 0 { println!("No saved texts"); }
                else { for i in 0..len { println!("  {}: \"{}\"", i, self.texts[len - i - 1]); } }
            },
            _ => {
                if let Some(num) = int_arg(words, 1, 0) {
                    if num >= len { println!("Number too large, no such text"); }
                    else {
                        let text = self.texts.remove(len - 1 - num);
                        println!("Using {:?}", text);
                        self.texts.push(text);
                    }
                } else { self.texts.push(input_substring(words, 1, 1000)); }
            },
        }
    }

    /// executes a **tree** command: parses and prints the tree for the current regular executes
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
    
    /// executes a **search** command: parses and prints the results for the current regexp and text
    fn do_search(&self, words: &Vec<&Report>) {
        let mut trace: usize = 0;
        let mut names = Vec::<String>::new();
        let mut all = false;
        let mut ptr = 1;
        if words.len() > 1 {
            if let Ok(num) = words[1].string().parse::<usize>() {
                trace = num;
                ptr += 1;
            }
            for word in words[ptr..].iter() {
                if word.string() == "*" { all = true; }
                names.push(word.string());
            }
        }
        match (self.re(), self.text()) {
            (None, Some(_)) => println!("No current regular expression"),
            (Some(_), None) => println!("No current text"),
            (None, None) => println!("No regular expression or text, add some and try again"),
            (Some(re), Some(text)) => {
                match parse_tree(re.re.as_str(), re.alt_parser) {
                    Err(err) => println!("Error parsing RE: {}", err.msg),
                    Ok(node) => {
                        set_trace(trace);
                        if let Err(msg) = Input::init(text.to_string(), Vec::new()) { println!("{}", msg); return; }
                        match walk_tree(&node, 0) {
                            Err(msg) => println!("Error: {}", msg),
                            Ok(None) => println!("No match"),
                            Ok(Some(path)) => {
                                let report = Report::new(&path);
                                if names.is_empty() { println!("{:?}", report.display(0))}
                                else if all {
                                    for (name, matches) in report.get_named() {
                                        print_named_match(name, &matches);
                                    }
                                } else {
                                    let reports = report.get_named();
                                    for name in names {
                                        if let Some(matches) =  reports.get(name.as_str()) {
                                            print_named_match(&name, matches);
                                        }
                                    }
                                }
                            }
                        }
                        set_trace(0);
                    }
                }
            }
        }
    }
}

fn print_named_match(name: &str, matches: &Vec<&Report>) {
    if matches.len() == 1 {
        print!("{}: ", name);
        print_one_named_match(matches.last().unwrap(), "");
    } else {
        println!("{} : ", name);
        matches.iter().for_each(|x| print_one_named_match(x, "    "));
    }
}
fn print_one_named_match(report: &Report, prefix: &str) {
    let byte_pos = report.byte_pos();
    let char_pos = report.char_pos();
    println!("{}\"{}\", byte position ({}, {}], char position [{}, {})",
             prefix, report.string(), byte_pos.0, byte_pos.1, char_pos.0, char_pos.1);
}    

/// gets user response to a question. It takes a list of potential reply strings and returns
/// the matching string if there is one
/// - prompt: prompt to display to user
/// - choices: acceptable choices
/// - dflt: the default value to choose for empty input
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

/// parses the command from the possibly abbreviated version passed in
fn get_command(candidates: &'static [&str], cmd: &str) -> &'static str {
    if cmd.is_empty() { "" } else {
        match candidates.iter().filter(|x| x.starts_with(cmd)).collect::<Vec<&&str>>()[..] {
            [] => "unrecognized",
            [x] => x,
            _ => "ambiguous"
        }
    }
}

/// gets the raw input string from the user input, retaining all whitespace characters
fn input_substring (words: &Vec<& Report>, from: usize, to: usize) -> String {
    let len = words.len();
    if from >= len { "".to_string() }
    else {
        let r0 = words[from]; 
        let r1 = if to < len { words[to] } else { words[len - 1] };
        Input::apply(|input| input.full_text[r0.byte_pos().0..r1.byte_pos().1].to_string())
    }
}

/// tries to interpret the given argument as an int
fn int_arg(words: &Vec<&Report>, arg_num: usize, dflt: usize) -> Option <usize> {
    let arg = input_substring(words, arg_num, arg_num);
    if arg.is_empty() { Some(dflt) }
    else if let Ok(num) = arg.parse::<usize>() { Some(num) }
    else { None }
}


    
