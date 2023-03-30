// TODO:
// - lazy: implement lazy evaluation
// - named substrings
// - refactor: especially in WALK there seems to be a lot of repeat code. Using traits I think a lot can be consolidated
// - more special chars: for now it just has \N for numeric and . for any, need to add $, ^, ascii, upper case, lower case, whitespace, ...
pub mod walk;
#[cfg(test)]
mod tests;

use std::str::Chars;
use crate::{pad, trace};

const PEEKED_SANITY_SIZE: usize = 20;           // sanity check: peeked stack should not grow large
const EFFECTIVELY_INFINITE: usize = 99999999;   // big number to server as a cap for *


#[derive(PartialEq)]
pub enum Node {Chars(CharsNode), SpecialChar(SpecialCharNode), And(AndNode), Or(OrNode), Set(SetNode), None, }
use core::fmt::Debug;
impl Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Node::None => "None".to_string(),
            _ => self.tree_node().unwrap().desc(0)
        })
    }
}

#[derive(Debug,Clone,Copy,PartialEq)] 
pub struct Limits {
    min: usize,
    max: usize,
    lazy: bool,
}

impl Default for Limits {
    fn default() -> Limits { Limits{min: 1, max: 1, lazy: false} }
}

impl Limits {
    fn simple_display(&self) -> String { format!("{{{},{}}}{}", self.min, self.max, if self.lazy {"L"} else {""})}

    fn parse(chars: &mut Peekable) -> Result<Limits, String> {
        let next = chars.next();
        if next.is_none() { return Ok(Limits::default()); }
        let next = next.unwrap();
        let (min, max): (usize, usize) = match next {
            '*' => (0, EFFECTIVELY_INFINITE),
            '+' => (1, EFFECTIVELY_INFINITE),
            '?' => (0, 1),
            '{' => Limits::parse_ints(chars)?,
            _ => { chars.put_back(next); return Ok(Limits::default()) },
        };
        let qmark = chars.peek();
        let lazy = qmark.unwrap_or('x') == '?';
        if lazy { let _ = chars.next(); }
        Ok(Limits{min, max, lazy})
    }

    fn parse_ints(chars: &mut Peekable) -> Result<(usize, usize), String> {
        let num = read_int(chars);
        let peek = chars.next();
        if num.is_none() || peek.is_none(){ return Err("Unterminated repetition block".to_string()); }
        let num = num.unwrap();
        match peek.unwrap() {
            '}'=> Ok((num, num)),
            ','=> {
                let n2 = if let Some(n) = read_int(chars) { n }
                else { EFFECTIVELY_INFINITE };
                let terminate = chars.next();
                if terminate.unwrap_or('x') != '}' {Err("Malformed repetition block error 1".to_string())}
                else {Ok((num, n2))}
            },
            _ => Err("Malformed repetition block error 2".to_string())
        }
    }

    // Checks if the size falls in the range.
    // Returns: <0 if NUM is < min; 0 if NUM is in the range min <= NUM <= ,ax (but SEE WARNING BELOW: NUM needs
    // to be adjusted to account for the 0-match possibility.
    //
    //Beware: the input is usize and is in general the length of steps vector.
    // This has a 0-match in its first position, so the value entered is actually one higher than the allowed value.
    pub fn check(&self, num: usize) -> isize {
        if num <= self.min { -1 }
        else if num <= self.max + 1 { 0 }
        else { 1 }
    }
    
    pub fn initial_walk_limit(&self) -> usize { if self.lazy {self.min} else { self.max}}
}
//
// These functions parse the reps option from the re source
//

fn read_int(chars: &mut Peekable) -> Option<usize> {
    let mut num: usize = 0;
    let mut any = false;
    loop {
        let digit = chars.next();
        if digit.is_none() { break; }
        let digit = digit.unwrap();
        if !('0'..='9').contains(&digit) {
            chars.put_back(digit);
            break;
        }
        any = true;
        num = num*10 + (digit as usize) - ('0' as usize);
    }
    if any { Some(num) } else { None }
}


pub const NODE_CHARS:    usize = 0;
pub const NODE_SPEC:     usize = 1;
pub const NODE_AND:      usize = 2;
pub const NODE_OR:       usize = 3;
pub const NODE_SET:      usize = 4;
//pub const NODE_SUCCESS:  usize = 5;
pub const NODE_NONE:     usize = 6;

impl Node {
    fn is_none(&self) -> bool { self.node_type() == NODE_NONE }
    fn node_type(&self) -> usize{
        match self {
            Node::Chars(_a)       => NODE_CHARS,
            Node::SpecialChar(_a) => NODE_SPEC,
            Node::And(_a)         => NODE_AND,
            Node::Or(_a)          => NODE_OR,
            Node::Set(_a)         => NODE_SET,
            Node::None            => NODE_NONE,
        }
    }

    fn walk<'a>(&'a self, string: &'a str) -> walk::Path<'a> {
        match self {
            Node::Chars(chars_node) => walk::CharsStep::walk(chars_node, string),
            Node::SpecialChar(special_node) => walk::SpecialStep::walk(special_node, string),
            Node::Set(set_node) => walk::SetStep::walk(set_node, string),
            Node::And(and_node) => walk::AndStep::walk(and_node, string),
            Node::Or(or_node) => walk::OrStep::walk(or_node, string),
            _ => panic!("TODO")
        }
    }
    //
    // Following are to fix special cases in building the tree. If I could redesign regexps they wouldn't be needed, but
    // to get the right behavior sometimes special tweaking is needed.
    //
    
    // This is to handle the case "XXX\|abcd" where a string of chars follows an AND. Only the first char should bind to the
    // OR. This method returns theany extra characters to the queue. If the Node is not Chars it does nothing.
    fn chars_after_or(&mut self, chars: &mut Peekable) {
        if let Node::Chars(chars_node) = self {
            while chars_node.string.len() > 1 {
                chars.put_back(chars_node.string.pop().unwrap());
            }
        }
    }

    // For the case abc\|XXX, break the preceding "abc" into "ab" and "c" since only the "c" binds with the OR
    fn chars_before_or(nodes: &mut Vec<Node>) {
        let prev = &nodes[nodes.len() - 1];
        if prev.node_type() == NODE_CHARS {
            let mut prev = nodes.pop().unwrap();
            let chars_node = prev.chars_mut_ref();
            if chars_node.string.len() > 1 {
                let new_node = Node::Chars(CharsNode {string: chars_node.string.pop().unwrap().to_string(), lims: Limits::default()});
                nodes.push(prev);
                nodes.push(new_node);
            } else {
                nodes.push(prev);
            }                
        }
    }

    // This handles the case where an OR is being inserted ino an AND.
    //  - If the preceding node is an OR this OR node gets discarded and its condition is appended to the
    //    existing one.
    //  - If the preceding node is not an OR then that node is removed from the AND list and inserted as the
    //    first element in the OR list.
    //  - In addition, if the preceding node is CHARS and its length is >1 it needs to be split, since the
    //    OR only binds a single character
    fn or_into_and(mut self, nodes: &mut Vec<Node>) {
        if nodes.is_empty() { return; }
        Node::chars_before_or(nodes);
        let mut prev = nodes.pop().unwrap();
        match prev.node_type() {
            NODE_OR => {
                let prev_node = prev.or_mut_ref();
                prev_node.push(self);
                prev_node.lims.max += 1;
                nodes.push(prev);
            },
            _ => {
                let or_node = self.or_mut_ref();
                or_node.push_front(prev);
                or_node.lims.max += 1;
                nodes.push(self);
            }
        }
    }

    //
    // Access Node content structs
    //
    
    // this gets a reference to the enclosed data as an immutable TreeNode. To get a ref to the object itself
    // use (or add) the methods below
    fn tree_node(&self) -> Option<&dyn TreeNode> {
        match self {
            Node::Chars(a)       => Some(a),
            Node::SpecialChar(a) => Some(a),
            Node::And(a)         => Some(a),
            Node::Or(a)          => Some(a),
            Node::Set(a)         => Some(a),
            Node::None           => None,
        }
    }
    // following are methods to access the different types by ref, muable and immutable. I wonder if it is possible
    // for a single templae to handle all cases?
    // These are for internal use, not API, so any bad call is a programming error, not a user error. That is why tey
    // panic rather than return Option
    fn or_ref(&self) -> &OrNode {
        match self {
            Node::Or(node) => node,
            _ => panic!("Attempting to get ref to OrNode from wrong Node type")
        }
    }
    fn and_ref(&self) -> &AndNode {
        match self {
            Node::And(node) => node,
            _ => panic!("Attempting to get ref to AndNode from wrong Node type")
        }
    }
    fn chars_ref(&self) -> Option<&CharsNode> {
        match self {
            Node::Chars(node) => Some(node),
            _ => None,
        }
    }
    // thank you rust-lang.org
    fn or_mut_ref(&mut self) -> &mut OrNode {
        match self {
            Node::Or(or_node) => or_node,
            _ => panic!("Attempting to get mut ref to OrNode from wrong Node type")
        }
    }
    fn and_mut_ref(&mut self)->&mut AndNode {
        match self {
            Node::And(and_node) => and_node,
            _ => panic!("Attempting to get mut ref to AndNode from wrong Node type")
        }
    }
    fn chars_mut_ref(&mut self)->&mut CharsNode {
        match self {
            Node::Chars(chars_node) => chars_node,
            _ => panic!("Attempting to get mut ref to CharsNode from wrong Node type")
        }
    }
}

//
// Treenodes are the structs used to build the parse tree. It is constructed in the first pass and then used
// to walk the search string in a second pass
//
//
// Node structure definitions: these all implement TreeNode
//

// handles strings of regular characters
// Since character strings are implicit ANDs the limit only applies if there is a single char in the string.
#[derive(Default, Debug, PartialEq)]
pub struct CharsNode {
    lims: Limits,
    string: String,
}

// handles special characters like ".", \d, etc.
#[derive(Default, Debug, PartialEq)]
pub struct SpecialCharNode {
    lims: Limits,
    special: char,
}

// handles AND (sequential) matches
#[derive(Default, Debug, PartialEq)]
pub struct AndNode {
    lims: Limits,
    nodes: Vec<Node>,
    report: bool,
    anchor: bool
}

// handles A\|B style matches
#[derive(Default, PartialEq, Debug)]
pub struct OrNode {
    nodes: Vec<Node>,
    // Limits for OR nodes are different than the others. ORs cannot be repeated (except by enclosing them in
    // an AND), so Limits is used for OR to move through the different branches rather than the different repetitions
    lims: Limits,
}

// handles [a-z] style matches
#[derive(Default, PartialEq, Debug)]
pub struct SetNode {
    lims: Limits,
    targets: Vec<Set>,
    not: bool,
}

//////////////////////////////////////////////////////////////////
//
// The TreeNode trait
//
// This section defines the TreeNode trait and includes the Node implementations
//
//////////////////////////////////////////////////////////////////

pub trait TreeNode {
    fn desc(&self, indent: usize) -> String;
    // looks for a single match, does not care about repeats
    fn matches(&self, string: &str) -> bool { string.is_empty() }
    fn limits(&self) -> Limits;
}

// CharsNode keeps a string of consecuive characters to look for. Really, it is like an AndNode where each node has a single
// character, but that seems wasteful/ he sring contains no special chars, and it can have no repeat count unless there is
// only a single character.
impl TreeNode for CharsNode {
    fn desc(&self, indent: usize) -> String { format!("{}CharsNode: '{}'{}", pad(indent), self.string, self.limits().simple_display()) }
    fn matches(&self, string: &str) -> bool { string.starts_with(&self.string) }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for SpecialCharNode {
    fn desc(&self, indent: usize) -> String {
        let slash = if ".$".contains(self.special) { "" } else { "\\" };
        format!("{}SpecialCharNode: '{}{}'{}", pad(indent), slash, self.special, self.limits().simple_display())
    }

    fn matches(&self, string: &str) -> bool {
        match string.chars().next() {
            Some(ch) => self.match_char(ch),
            None => self.special == '$'    // match only end-of-string marker
        }
    }

    fn limits(&self) -> Limits { self.lims }
}

    
// format the limis for debugging
impl TreeNode for AndNode {
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}AndNode {} {}", pad(indent), self.limits().simple_display(), if self.report {"report"} else {""});
        for i in 0..self.nodes.len() {
            let disp_str = match self.nodes[i].tree_node() {
                Some(node) => node.desc(indent + 1),
                None => format!("{:?}", self.nodes[i]),
            };
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for OrNode {
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}OrNode{}", pad(indent), self.limits().simple_display());
        for i in 0..self.nodes.len() {
            let disp_str = match self.nodes[i].tree_node() {
                Some(node) => node.desc(indent + 1),
                None => format!("{:?}", self.nodes[i]),
            };
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for SetNode {
    fn desc(&self, indent: usize) -> String {
        format!("{}Set {}{}", pad(indent), self.targets_string(), self.limits().simple_display(), )
    }
    fn limits(&self) -> Limits { self.lims }
}


//////////////////////////////////////////////////////////////////
//
// Node, other implementations
//
// The most important is that each one needs to define a contructor taking the Peekable as input
// and returning a Node enum element (complete with its TreeNode filling)
//
//////////////////////////////////////////////////////////////////


impl CharsNode {
    // These characters have meaning when escaped, break for them. Otherwise just delete the '/' from the string
    // TODO: get the right codes
    const ESCAPE_CODES: &str = "dula()|";

    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut chs = Vec::<char>::new();
        loop {
            match chars.peek_2() {
                (Some(ch0), Some(ch1)) => {
                    // only break on '$' if it is the last char - skip over the trailing '\)'
                    if '$' == ch0 && chars.peek_n(4)[3].is_none() { break; }
                    if r".[".contains(ch0) { break; }
                    if ch0 == '\\' {
                        if "()|".contains(ch1) { break; }
                        if CharsNode::ESCAPE_CODES.contains(ch1) { break; }
                        let _ = chars.next();    // pop off the '/'
                    } else if "?*+{".contains(ch0) {  // it is a rep count - this cannot apply to a whole string, just a single character
                        if chs.len() > 1 {           // so return the previous character to the stream and register the rest
                            chars.put_back(chs.pop().unwrap());   // TODO: can chs be empty?
                        }
                        break;
                    }
                },
                (Some(_ch0), None) => {
                    return Err("Bad escape char".to_string());
                },
                _ => { break; }
            }
            chs.push(chars.next().unwrap());
        }
        Ok(if chs.is_empty() { Node::None }
           else {
               let lims = if chs.len() == 1 { Limits::parse(chars)? } else { Limits::default() };
               Node::Chars(CharsNode {
                   string: chs.into_iter().collect(),
                   lims
               })
           })
    }
}    

impl SpecialCharNode {
    // called with pointer at a special character. Char can be '.' or "\*". For now I'm assuming this only gets called with special
    // sequences at bat, so no checking is done.
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let special = if let Some(ch) = chars.next() {
            if ".$".contains(ch) { ch }
            else if let Some(_ch1) = chars.peek() { chars.next().unwrap() }   // ch1 is next(), doing it this way gets the same value and pops it off
            else { return Ok(Node::None); }
        } else { return Ok(Node::None); };
        Ok(Node::SpecialChar(SpecialCharNode { special, lims: Limits::parse(chars)?}))
    }

    fn match_char(&self, ch: char) -> bool {
        match self.special {
            '.' => true,                        // all
            'd' => '0' <= ch && ch <= '9',      // numeric
            'l' => 'a' <= ch && ch <= 'z',      // lc ascii
            'u' => 'A' <= ch && ch <= 'Z',      // uc ascii
            'a' => ' ' <= ch && ch <= '~',      // ascii printable
            _ => false
        }
    }
}

impl AndNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }

    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let report = chars.peek().unwrap_or('x') != '?';
        if !report { let _ = chars.next(); }
        let mut nodes = Vec::<Node>::new();
        loop {
            let (ch0, ch1) = chars.peek_2();
            if ch0.is_none() { return Err("Unterminated AND node".to_string()); }
            if ch0.unwrap() == '\\' && ch1.unwrap_or('x') == ')' { break; }
            let node = parse(chars)?;
            match node.node_type() {
                NODE_NONE => (),
                NODE_OR => node.or_into_and(&mut nodes),
                _ => nodes.push(node),
            }
        }
        // pop off terminating chars
        let _ = chars.next();
        let _ = chars.next();
        Ok(if nodes.is_empty() { Node::None }
           else { Node::And(AndNode {nodes, lims: Limits::parse(chars)?, report, anchor: false, })})
    }
}

impl OrNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }
    fn push_front(&mut self, node: Node) { self.nodes.insert(0, node); }
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut nodes = Vec::<Node>::new();
        let mut node = parse(chars)?;
        if !node.is_none() {
            // only the first char in a character string should be in an OR
            node.chars_after_or(chars);
            nodes.push(node);
        }
        Ok(Node::Or(OrNode {nodes, lims: Limits {min: 0, max: 0, lazy: false}}))
    }
    fn fix_limits(&mut self) {
        let max = self.nodes.len() - 1;
        self.lims = Limits {min: 0, max, lazy: false};
    }
}

impl SetNode {
    fn push(&mut self, set: Set) { self.targets.push(set); }
    
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut targets = Vec::<Set>::new();
        let mut not = false;
        if let Some(ch) = chars.peek() {
            if ch == '^' {
                let _ = chars.next();
                not = true;
            }
        }
        loop {
            match chars.peek() {
                None => break,
                Some(ch) => {
                    if ch == ']' { break; }
                    let target = Set::parse_next(chars)?; 
                    if target != Set::Empty {targets.push(target) }
                },
            }
        }
        
        // eiher None or ']'
        if let Some(ch) = chars.next() { if ch != ']' {return Err("Unterminated Set".to_string()); } };
        Ok(if targets.is_empty() { Node::None }
           else { Node::Set( SetNode {targets, not, lims: Limits::parse(chars)?}) })
    }
    
    fn matches(&self, string: &str) -> bool {
        match string.chars().next() {
            Some(ch) => self.match_char(ch),
            None => false
        }
    }
    
    fn match_char(&self, ch: char) -> bool { self.not != self.targets.iter().any(move |x| x.matches(ch)) }
    fn targets_string(&self) -> String {
        format!("[{}{}]", if self.not {"^"} else {""}, self.targets.iter().map(|x| x.desc()).collect::<Vec<_>>().join(""))
    }
}    
// used to mark ranges for SetNode
// TODO: maybe combine single chars into String so can use contains() to ge all at once?
#[derive(Debug,PartialEq)]
enum Set {RegularChars(String), SpecialChar(char), Range(char, char), Empty}

impl Set {
    fn desc(&self) -> String {
        match self {
            Set::RegularChars(string) => string.to_string(),
            Set::SpecialChar(ch) => format!("\\{}", ch),
            Set::Range(ch0, ch1) => format!("{}-{}", ch0, ch1),
            Set::Empty => "*EMPTY*".to_string(),
        }
    }
    fn parse_next(chars: &mut Peekable) -> Result<Set, String> {
        Ok(match chars.peek_2() {
            (Some(ch0), Some(ch1)) => {
                if ch1 == '-'{
                    Set::Range(chars.next().unwrap(), (chars.next(), chars.next().unwrap()).1)
                } else if ch0 == '\\' {
                    Set::SpecialChar((chars.next(), chars.next()).1.unwrap())
                } else {
                    let mut string = "".to_string();
                    loop {
                        match chars.peek_2() {
                            (Some(ch0), Some(ch1)) => {
                                if ch0 == ']' || ch0 == '\\' || ch1 == '-' { break; }
                                string.push(chars.next().unwrap());
                            },
                            _ => { return Err("Unterminated set block".to_string()); },
                        }
                    }
                    if string.is_empty() {Set::Empty} else {Set::RegularChars(string)}
                }
            }
            _ => Set::Empty,
        })
    }
    fn matches(&self, ch: char) -> bool {
        match self {
            Set::RegularChars(string) => string.contains(ch),
            Set::SpecialChar(_ch) => false,   // TODO
            Set::Range(ch0, ch1) => *ch0 <= ch && ch <= *ch1,
            Set::Empty => false,
        }
    }
}

//////////////////////////////////////////////////////////////////
//
// parse()
//
// This gets its own section because it is the core of the tree parser. It peeks at the
// next char or so, decides what it is, and calls the proper parse_node to parse the enext node.
//
//////////////////////////////////////////////////////////////////

fn parse(chars: &mut Peekable) -> Result<Node, String> {
    let (ch0, ch1) = chars.peek_2();
    if ch0.is_none() { return Ok(Node::None); }
    let ch0 = ch0.unwrap();
    let ch1 = ch1.unwrap_or(' ');  // SPACE isn't in any special character sequence
    if ch0 == '[' {
        let _ = chars.next();
        SetNode::parse_node(chars)
    } else if ch0 == '\\' {
        if ch1 != '(' && ch1 != '|' {
            if ch1 == '\\' {
                CharsNode::parse_node(chars)
            } else {
                SpecialCharNode::parse_node(chars)
            }
        } else {
            let _ = chars.next();
            let _ = chars.next();
            if ch1 == '(' {
                AndNode::parse_node(chars)
            } else {
                OrNode::parse_node(chars)
            }
        }
    } else if ".$".contains(ch0) {
        SpecialCharNode::parse_node(chars)
        } else {
        CharsNode::parse_node(chars)
    }
}

//
// Main entry point for parsing tree
//
// Wraps the in put with "\(...\)" so it becomes an AND node, and sticks the SUCCESS node on the end when done
pub fn parse_tree(input: &str) -> Result<Node, String> {
    // wrap the string in "\(...\)" to make it an implicit AND node
    let anchor_front = input.starts_with('^');
    let mut chars = Peekable::new(&input[(if anchor_front {1} else {0})..]);
    chars.push('\\');
    chars.push(')');
    let mut outer_and = AndNode::parse_node(&mut chars)?;
    if anchor_front {
        let and_node = outer_and.and_mut_ref();
        and_node.anchor = true;
    }
    Ok(outer_and)
}

pub fn walk_tree<'a>(tree: &'a Node, text: &'a str) -> Option<walk::Path<'a>> {
    let mut start = text;
    // hey, optimization
    // deosn't save that much time but makes the trace debug easier to read
    let root = tree.and_ref();
    if !root.anchor {
        if let Some(node_0) = tree.and_ref().nodes[0].chars_ref() {
            if node_0.limits().min > 0 {
                let copy = node_0.string.to_string();
                match start.find(&copy) {
                    Some(offset) => {
                        if offset > 0 {
                            if trace(1) { println!("\nOptimization: RE starts with \"{}\", skipping {} bytes", node_0.string, offset); }
                            start = &start[offset..];
                        }
                    },
                    None => { return None; }
                }
            };
        }
    }
    while !start.is_empty() {
        if trace(1) {println!("\n==== WALK \"{}\" ====", start)};
        let path = tree.walk(start);
        if path.len() > 1 {
            if trace(1) { println!("--- Search succeeded ---") };
            return Some(path);
        }
        if trace(1) {println!("==== WALK \"{}\": no match ====", start)};
        if tree.and_ref().anchor { break; }
        start = &start[char_bytes(start, 1)..];
    }
    None
}
//////////////////////////////////////////////////////////////////
//
// Helper functions
//
//////////////////////////////////////////////////////////////////

// gets the number of bytes in a sring of unicode characters
fn char_bytes(string: &str, char_count: usize) -> usize {
    let s: String = string.chars().take(char_count).collect();
    s.len()
}

//////////////////////////////////////////////////////////////////
//
// Peekable
//
// This is an iterator with added features to make linear parsing of the regexp string easier:
//     1) peeking: the next char can be peeked (read without consuming) or returned after being consumed
//     2) extra characters can be added to the stream at the end of the buffer (without copying the entire string)
//
// It also has progress(), a sanity check to catch suspicious behavior, like infinite loops or overuse of peeking
//
//////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct Peekable<'a> {
    chars: Chars<'a>,
    peeked: Vec<char>,
    trailer: Vec<char>,
    progress_check: isize,
}

impl<'a> Peekable<'a> {
    fn new(string: &str) -> Peekable { Peekable { chars: string.chars(), peeked: Vec::<char>::new(), trailer: Vec::<char>::new(), progress_check: 1} }

    pub fn next(&mut self) -> Option<char> {
        if !self.peeked.is_empty() { Some(self.peeked.remove(0)) }
        else { self.next_i() }
    }

    // peek() looks at the next character in the pipeline. If called multiple times it returns the same value
    pub fn peek(&mut self) -> Option<char> {
        if self.peeked.is_empty() {
            let ch = self.next_i()?;
            self.peeked.push(ch);
        }
        Some(self.peeked[0])
    }

    // peek at the next n chars
    pub fn peek_n(&mut self, n: usize) -> Vec<Option<char>> {
        let mut ret: Vec<Option<char>> = Vec::new();
        for ch in self.peeked.iter() {
            if ret.len() == n { return ret; }
            ret.push(Some(*ch));
        }
        while ret.len() < n { ret.push(self.peek_next()); }
        ret
    }

    // convenient because 2 chars is all the lookahead I usually need
    pub fn peek_2(&mut self) -> (Option<char>, Option<char>) {
        let x = self.peek_n(2);
        (x[0], x[1])
    }


    // This simply adds the char back in the queue. It is assumed the caller returns the chars in the reverse order they are popped off
    pub fn put_back(&mut self, ch: char) {
        self.progress_check -= 1;
        self.peeked.insert(0, ch);
    }
    
    pub fn push(&mut self, ch: char) { self.trailer.push(ch); }


    // simple to do, and maybe useful for early stages: make sure the parse loop can't get through without burning at least one character
    fn progress(&mut self) {
        if self.progress_check <= 0 {panic!("Looks like no progress is being made in parsing string"); }
        if self.peeked.len() > PEEKED_SANITY_SIZE { panic!("PEEKED stack has grown to size {}", self.peeked.len()); }
        self.progress_check = 0;
    }
    
    fn next_i(&mut self) -> Option<char> {
        let mut ret = self.chars.next();
        if ret.is_none() {
            ret = if self.trailer.is_empty() { None } else { Some(self.trailer.remove(0)) };
        }
        self.progress_check += 1;
        ret
    }
            
    // peek_next() gets the next unread character, adds it to the peeked list, and returns it
    fn peek_next(&mut self) -> Option<char> {
        let ch = self.next_i();
        if let Some(c) = ch { self.peeked.push(c); }
        ch
    }

}

