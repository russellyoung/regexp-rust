use crate::regexp::*;
use crate::set_trace;
use crate::Config;
use std::io;
use std::io::Write;    
use std::collections::HashMap;

const PROMPT: &str = "> ";

pub struct Interactive {
    res: Vec<String>,
    texts: Vec<String>,
    tree: Node,
    cmd_parse_tree: Node,
    prompt_str: String,
    abbrev: u32,
}

const CMD_PARSE_RE:&str = r"^ *\(?<all>\(?\(?<cmd>[rtfwh?][a-z]*\)[^a-z]\|$\) *\(?<body>\(?\(?<subcmd>[a-z]+\)[^a-z]\|$\)\|\(?<num>[0-9]*\)[^0-9]\|$ *\(?<tail>.*\)\)?\)";
const HELP_TEXT: &str = r"
This is an interactive interface to the regexp search engine. The program keeps stacks of
regular expressions and search texts and uses them to run searches. Besides simple searching
the program will print out the parsed search tree and also details of the walk over the target
string.

Commands are in general of the form CMD [SUBCMD [DATA]], though it will try to guess the 
meaning of ambiguous commands. The commands and subcommands can be abbreviated with the 
first couple unique letters.

The commands are:
 - re: 
   -   display the current active regular expression
   - re [set] REGULAREXPRESSION: sets a new regular expression to be the current one. The 
       SET keyword is optional, if not given the program usually will guess the text is
       intended as a regular expression. Only in the rare case when the text starts with a 
       keyword is the SET subcommand required.
   - re history: lists the most recent regular expressions
   - re NUMBER: sets the NUMBERth item on the history list to be the current regular expression
   - re pop: pops off (deletes) the current re from the list
 - text:
   - displays the current active text string
   - text TEXT: sets a new regular expression to be the current one. The 
       SET keyword is optional, if not given the program usually will guess the text is
       intended as a regular expression. Only in the rare case when the text starts with a 
       keyword is the SET subcommand required.
   - text history: lists the most recent regular expressions
   - text NUMBER: sets the NUMBERth item on the history list to be the current regular expression
   - text pop: pops off (deletes) the current re from the list
 - find: performs a RE search using the current RE and the current text
 - tree: displays the parse tree created in the first stage of the RE search using the current RE
 - walk: displays the progress as the tree is walked in stage 2 of the search
 - help: displays this help
";
fn get_var<'a>(vars: &HashMap<&'a str, Vec<&'a Report>>, name: &'a str) -> &'a str {
    if let Some(var) = vars.get(name) { var[0].found.as_str() } else { "" }
}

impl Interactive {
    pub fn new(config: Config) -> Interactive {
        let mut res = Vec::<String>::new();
        if !config.re.is_empty() { res.push(config.re.to_string()); }
        let mut texts = Vec::<String>::new();
        if !config.text.is_empty() { texts.push(config.text.to_string()); }
        Interactive { res,
                      texts,
                      tree: Node::None,
                      cmd_parse_tree: parse_tree(CMD_PARSE_RE).unwrap(),
                      prompt_str: PROMPT.to_string(),
                      abbrev: config.abbrev,
        }
    }
    
    pub fn run(&mut self) {
        let stdin = io::stdin();
        let mut buffer;
        loop {
            self.prompt();
            buffer = "".to_string();
            match stdin.read_line(&mut buffer) {
                Ok(0) => { break; },
                Ok(1) => (),
                Ok(_x) => {
                    let  _ = buffer.pop();
                    if !self.do_command(&buffer) {break; }
                },
                Err(_msg) => { break;},
            }
        }
        println!("exit");
    }

    fn guess_re(&mut self, maybe_re: &str) -> bool {
        if (maybe_re.contains('\\') || maybe_re.contains('*') || maybe_re.contains('+'))
            && yorn(&format!("\"{}\" looks like a RE. Is it?", maybe_re), Some(true)) {
                self.res.push(maybe_re.to_string());
                true
            }
        else { false }
    }
    
    fn prompt(&mut self) {
        print!("{} ", self.prompt_str);
        std::io::stdout().flush().unwrap();
    }

    fn do_command(&mut self, input: &str) -> bool{
        let walk = walk_tree(&self.cmd_parse_tree, input);
        if let Ok(Some((path, _, _))) = &walk {
            let report = Report::new(path, 0, 0);
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
                                 get_var(&vars, "body"),     // everything after the first word
                                 get_var(&vars, "all"),)     // everything, trimmed
        } else {
            self.execute_command("", "", "", "", "", input)
        }
    }

    fn execute_command(&mut self, cmd: &str, subcmd: &str, num: &str, tail: &str, body: &str, all: &str) -> bool{
        //println!("cmd: '{}', subcmd: '{}', num: '{}', tail: '{}', body: '{}', all: '{}'", cmd, subcmd, num, tail, body, all);
        if cmd.is_empty() {
            if !self.guess_re(all) { println!("Unrecognized command"); }
        } else if "re".starts_with(cmd) {self.do_re(subcmd, num, tail, body); }
        else if "text".starts_with(cmd) { self.do_text(subcmd, num, tail, body); }
        else if "tree".starts_with(cmd) {
            if self.res.is_empty() { println!("No current RE, first enter one"); }
            else {
                match parse_tree(&self.res[self.res.len() - 1]) {
                    Ok(node) => println!("--- Parse tree:\n{:?}", node),
                    Err(error) => println!("Error parsing tree: {}", error),
                }
            }
        } else if "find".starts_with(cmd) {
            let re = {if !self.res.is_empty() { &self.res[self.res.len() - 1] } else { println!("No current RE"); return true; }};
            let text = {if !self.texts.is_empty() { &self.texts[self.texts.len() - 1] } else { println!("No current text"); return true; }};
            match parse_tree(re) {
                Ok(node) => {
                    match walk_tree(&node, text) {
                        Ok(Some((path, char_start, bytes_start))) => println!("{:?}", Report::new(&path, char_start, bytes_start).display(0)),
                        Ok(None) => println!("No match"),
                        Err(error) => println!("Error in search: {}", error)
                    }
                },
                Err(error) => { println!("Error parsing tree: {}", error); return true; },
            }
        } else if "walk".starts_with(cmd) {
            let re = {if !self.res.is_empty() { &self.res[self.res.len() - 1] } else { println!("No current RE"); return true; }};
            let text = {if !self.texts.is_empty() { &self.texts[self.texts.len() - 1] } else { println!("No current text"); return true; }};
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
                Err(error) => { println!("Error parsing tree: {}", error); return true; },
            }
        } else if "help".starts_with(cmd) || cmd == "?" { println!("{}", HELP_TEXT); }
        else if "quit".starts_with(cmd) || ("exit".starts_with(cmd) && yorn("Really exit?", Some(true))) { return false; }
        true
    }

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
                if self.res.is_empty() { println!("No current RE"); }
                else { println!("current RE is \"{}\"", self.res[self.res.len() - 1]); }
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
                if self.texts.is_empty() { println!("No current text"); }
                else { println!("current text is \"{}\"", self.texts[self.texts.len() - 1]); }
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
