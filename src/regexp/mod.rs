// TODO:
// - lazy: implement lazy evaluation
// - named substrings
// - refactor: especially in WALK there seems to be a lot of repeat code. Using traits I think a lot can be consolidated
// - more special chars: for now it just has \N for numeric and . for any, need to add $, ^, ascii, upper case, lower case, whitespace, ...
pub mod walk;
#[cfg(test)]
mod tests;

use std::str::Chars;
use crate::{pad, trace, trace_indent, trace_change_indent, trace_set_indent};
use core::fmt::{Debug,};
use std::collections::HashMap;

const PEEKED_SANITY_SIZE: usize = 20;           // sanity check: peeked stack should not grow large
const EFFECTIVELY_INFINITE: usize = 99999999;   // big number to server as a cap for *

//////////////////////////////////////////////////////////////////
//
// Node
//
// Nodes act as a container to hold the TreeNodes that make up the tree. At first I used Box for everything but that
// made it hard to keep track of what was what, eventually I thought if using enums as wrapper. That makes passing
// things around conveneint, though it does require some way ofgetting back the TreeNode object. I've looked at
// making all the Node types hold dyn TreeNode, which would make fetching them easier, but there still do seem to
// be some places where I need to access the full object. It looks like a problem with using enums as Box is that
// the size must be set at compile time and that cannot be done for traits
//
// I also considered using a union for the contents - that is another way besides trait of having the same contents
// type for everything. I think that would work well, but in the end I don't see a big advantage over using enums.
//
//////////////////////////////////////////////////////////////////
#[derive(PartialEq)]
pub enum Node {Chars(CharsNode), SpecialChar(SpecialCharNode), And(AndNode), Or(OrNode), Set(SetNode), None, }

impl core::fmt::Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", if let Some(node) = self.tree_node() { node.desc(0) } else { "None".to_string() })
    }
}

impl Node {
    fn is_none(&self) -> bool { *self == Node::None }
    fn walk<'a>(&'a self, string: &'a str) -> walk::Path<'a> {
        match self {
            Node::Chars(chars_node) => walk::CharsStep::walk(chars_node, string),
            Node::SpecialChar(special_node) => walk::SpecialStep::walk(special_node, string),
            Node::Set(set_node) => walk::SetStep::walk(set_node, string),
            Node::And(and_node) => walk::AndStep::walk(and_node, string),
            Node::Or(or_node) => walk::OrStep::walk(or_node, string),
            Node::None => panic!("NONE node should not be in final tree")
        }
    }

    // Get the node object from inside its enum wrapper
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
    // These are for internal use, not API, so any bad call is a programming error, not a user error. That is why tey
    // panic rather than return Option
    // thank you rust-lang.org
    fn mut_or_ref(&mut self)   -> &mut OrNode    { if let Node::Or(node)    = self { node } else { panic!("Trying for mut ref to OrNode from wrong Node type"); } }
    fn mut_and_ref(&mut self)  -> &mut AndNode   { if let Node::And(node)   = self { node } else { panic!("Trying for mut ref to AndNode from wrong Node type"); } }
    fn mut_chars_ref(&mut self)-> &mut CharsNode { if let Node::Chars(node) = self { node } else { panic!("Trying for mut ref to CharsNode from wrong Node type"); } }

    //
    // Following are to handle special cases in building the tree. If I could redesign regexps they wouldn't be needed, but
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
        let prev_is_chars = matches!(&nodes[nodes.len() - 1], Node::Chars(_));
        if prev_is_chars {
            let mut prev = nodes.pop().unwrap();
            let chars_node = prev.mut_chars_ref();
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
        if let Node::Or(_) = prev {
            let mut prev_node = prev.mut_or_ref();
            prev_node.push(self);
            prev_node.lims.max += 1;
            nodes.push(prev);
        } else {
            let or_node = self.mut_or_ref();
                or_node.push_front(prev);
                or_node.lims.max += 1;
                nodes.push(self);
        }
    }

    fn trace(self) -> Self {
        if trace(2) {
            match (&self, &self) {
                (Node::And(_), _) | (_, Node::Or(_)) => trace_change_indent(-1),
                _ => ()
            }
            println!("{}Created {:?}", trace_indent(), self);
        }
        self
    }
}

fn entering(name: &str, chars: &mut Peekable) {
    let chs = chars.peek_n(3);
    println!("{}{} starting from \"{}{}{}\"", trace_indent(), name, chs[0].unwrap(), chs[1].unwrap(), chs[2].unwrap_or(' '));
    if name == "AND" || name == "OR" { trace_change_indent(1); }
}

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
    // NAMED == None means do not report, NAMED == "" means unnamed 
    named: Option<String>,
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
// This section defines the TreeNode trait and includes the Node implementations. Actually,
// as it turned out, the TreeNode trait is not really useful. The first pass of the parser
// did use it, but that was before using the Node enum as a wrapper. With that there is not
// much gained by using the trait.
//
//////////////////////////////////////////////////////////////////

pub trait TreeNode {
    fn desc(&self, indent: isize) -> String;
    // looks for a single match, does not care about repeats
    fn matches(&self, string: &str) -> bool { string.is_empty() }
    fn limits(&self) -> Limits;
}

// CharsNode keeps a string of consecuive characters to look for. Really, it is like an AndNode where each node has a single
// character, but that seems wasteful/ he sring contains no special chars, and it can have no repeat count unless there is
// only a single character.
impl TreeNode for CharsNode {
    fn desc(&self, indent: isize) -> String { format!("{}CharsNode: '{}'{}", pad(indent), self.string, self.limits().simple_display()) }
    fn matches(&self, string: &str) -> bool { string.starts_with(&self.string) }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for SpecialCharNode {
    fn desc(&self, indent: isize) -> String {
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
    fn desc(&self, indent: isize) -> String {
        let name = { if let Some(name) = &self.named { format!("<{}>", name)} else {"".to_string()}};
        let mut msg = format!("{}AndNode({}) {} {}", pad(indent), self.nodes.len(), self.limits().simple_display(), name);
        for i in 0..self.nodes.len() {
            let disp_str = { if let Some(node) = self.nodes[i].tree_node() { node.desc(indent + 1) } else { format!("{:?}", self.nodes[i]) }};
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for OrNode {
    fn desc(&self, indent: isize) -> String {
        let mut msg = format!("{}OrNode{}", pad(indent), self.limits().simple_display());
        for i in 0..self.nodes.len() {
            let disp_str = { if let Some(node) = self.nodes[i].tree_node() { node.desc(indent + 1) } else { format!("{:?}", self.nodes[i]) }};
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
    fn limits(&self) -> Limits { self.lims }
}

impl TreeNode for SetNode {
    fn desc(&self, indent: isize) -> String {
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
    const ESCAPE_CODES: &str = "ntdula()|";

    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { entering("CHARS", chars); }
        let mut chs = Vec::<char>::new();
        loop {
            match chars.peek_2() {
                (Some(ch0), Some(ch1)) => {
                    // only break on '$' if it is the last char - skip over the trailing '\)'
                    if '$' == ch0 && chars.peek_n(4)[3].is_none() { break; }
                    if r".[".contains(ch0) { break; }
                    if ch0 == '\\' {
                        if CharsNode::ESCAPE_CODES.contains(ch1) { break; }
                        let _ = chars.next();    // pop off the '/'
                    } else if "?*+{".contains(ch0) {  // it is a rep count - this cannot apply to a whole string, just a single character
                        if chs.len() > 1 {           // so return the previous character to the stream and register the rest
                            chars.put_back(chs.pop().unwrap());   // TODO: can chs be empty?
                        }
                        // repeat count char at start of string is not special
                        if !chs.is_empty() {break};
                    }
                },
                (Some(_ch0), None) => {
                    return Err(Error::make(1, "Bad escape char"));
                },
                _ => { break; }
            }
            chs.push(chars.next().unwrap());
        }
        Ok((if chs.is_empty() { Node::None }
           else {
               let lims = if chs.len() == 1 { Limits::parse(chars)? } else { Limits::default() };
               Node::Chars(CharsNode {
                   string: chs.into_iter().collect(),
                   lims
               })
           }).trace())
    }
}    

impl SpecialCharNode {
    // called with pointer at a special character. Char can be '.' or "\*". For now I'm assuming this only gets called with special
    // sequences at bat, so no checking is done.
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { entering("SPECIAL", chars); }
        let special = if let Some(ch) = chars.next() {
            if ".$".contains(ch) { ch }
            else if let Some(_ch1) = chars.peek() { chars.next().unwrap() }   // ch1 is next(), doing it this way gets the same value and pops it off
            else { return Ok(Node::None); }
        } else { return Ok(Node::None); };
        Ok(Node::SpecialChar(SpecialCharNode { special, lims: Limits::parse(chars)?}).trace())
    }

    fn match_char(&self, ch: char) -> bool {
        match self.special {
            '.' => true,                        // all
            'd' => ('0'..='9').contains(&ch),   // numeric
            'l' => ('a'..='z').contains(&ch),   // lc ascii
            'u' => ('A'..='Z').contains(&ch),   // uc ascii
            'a' => (' '..='~').contains(&ch),   // ascii printable
            'n' => ch == '\n',                  // newline
            't' => ch == '\t',                  // tab
            _ => false
        }
    }
}

impl AndNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }

    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { entering("AND", chars); }
        let named = AndNode::parse_named(chars)?;
        let mut nodes = Vec::<Node>::new();
        loop {
            let (ch0, ch1) = chars.peek_2();
            if ch0.is_none() { return Err(Error::make(2, "Unterminated AND node")); }
            if ch0.unwrap() == '\\' && ch1.unwrap_or('x') == ')' { break; }
            let node = parse(chars)?;
            match node {
                Node::None => (),
                Node::Or(_) => node.or_into_and(&mut nodes),
                _ => nodes.push(node),
            }
        }
        // pop off terminating chars
        let _ = chars.next();
        let _ = chars.next();
        Ok((if nodes.is_empty() { Node::None }
           else { Node::And(AndNode {nodes, lims: Limits::parse(chars)?, named, anchor: false, })}).trace())
    }

    fn parse_named(chars: &mut Peekable) -> Result<Option<String>, Error> {
        match chars.peek_2() {
            (Some('?'), Some('<')) => {
                let (_, _) = (chars.next(), chars.next());
                let mut chs = Vec::<char>::new();
                loop {
                    match (chars.next(), chars.peek()) {
                        (Some('>'), _) => { break; },
                        (Some('\\'), Some(')')) => { return Err(Error::make(7, "Unterminated collect name in AND definition")); },
                        (Some(ch), _) => chs.push(ch),
                        _ => { return Err(Error::make(8, "error getting name in AND definition")); },
                    }
                }
                Ok(Some(chs.into_iter().collect()))
            },
            (Some('?'), _) => {
                let _ = chars.next();
                Ok(None)
            },
            _ => Ok(Some("".to_string())),
        }
    }
}

impl OrNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }
    fn push_front(&mut self, node: Node) { self.nodes.insert(0, node); }
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { entering("OR", chars); }
        let mut nodes = Vec::<Node>::new();
        let mut node = parse(chars)?;
        if !node.is_none() {
            // only the first char in a character string should be in an OR
            node.chars_after_or(chars);
            nodes.push(node);
        }
        Ok(Node::Or(OrNode {nodes, lims: Limits {min: 0, max: 0, lazy: false}}).trace())
    }
    fn fix_limits(&mut self) {
        let max = self.nodes.len() - 1;
        self.lims = Limits {min: 0, max, lazy: false};
    }
}

impl SetNode {
    fn push(&mut self, set: Set) { self.targets.push(set); }
    
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { entering("SET", chars); }
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
        if let Some(ch) = chars.next() { if ch != ']' {return Err(Error::make(3, "Unterminated Set")); } };
        Ok((if targets.is_empty() { Node::None }
           else { Node::Set( SetNode {targets, not, lims: Limits::parse(chars)?}) }).trace())
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
    fn parse_next(chars: &mut Peekable) -> Result<Set, Error> {
        let peeks = chars.peek_n(3);
        Ok(match (peeks[0], peeks[1], peeks[2]) {
            (Some(ch0), Some(ch1), Some(_ch2)) => {
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
                            _ => { return Err(Error::make(4, "Unterminated set block"));},
                        }
                    }
                    if string.is_empty() {Set::Empty} else {Set::RegularChars(string)}
                }
            }
            _ => {return Err(Error::make(4, "Unterminated set block"));},
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

fn parse(chars: &mut Peekable) -> Result<Node, Error> {
    let (ch0, ch1) = chars.peek_2();
    if ch0.is_none() { return Ok(Node::None); }
    let ch0 = ch0.unwrap();
    let ch1 = ch1.unwrap_or(' ');  // SPACE isn't in any special character sequence
    if ch0 == '[' {
        let _ = chars.next();
        SetNode::parse_node(chars)
    } else if ch0 == '\\' {
        if ch1 != '(' && ch1 != '|' {
            if ch1 == '\\' { CharsNode::parse_node(chars) }
            else { SpecialCharNode::parse_node(chars) }
        } else {
            let _ = chars.next();
            let _ = chars.next();
            if ch1 == '(' { AndNode::parse_node(chars) }
            else { OrNode::parse_node(chars) }
        }
    } else if ".$".contains(ch0) { SpecialCharNode::parse_node(chars) }
    else { CharsNode::parse_node(chars) }
}

//
// Main entry point for parsing tree
//
// Wraps the in put with "\(...\)" so it becomes an AND node, and sticks the SUCCESS node on the end when done
pub fn parse_tree(input: &str) -> Result<Node, Error> {
    trace_set_indent(0);
    // wrap the string in "\(...\)" to make it an implicit AND node
    let anchor_front = input.starts_with('^');
    let mut chars = Peekable::new(&input[(if anchor_front {1} else {0})..]);
    chars.push('\\');
    chars.push(')');
    let mut outer_and = AndNode::parse_node(&mut chars)?;
    if anchor_front {
        let and_node = outer_and.mut_and_ref();
        and_node.anchor = true;
    }
    match chars.next() {
        Some(_) => Err(Error::make(5, "Extra characters after parse completed")),
        None => Ok(outer_and),
    }
}

pub fn walk_tree<'a>(tree: &'a Node, text: &'a str) -> Result<Option<(walk::Path<'a>, usize, usize)>, Error> {
    trace_set_indent(0);
    let mut start = text;
    let mut start_pos = 0;
    // hey, optimization
    // deosn't save that much time but makes the trace debug easier to read
    let root = {if let Node::And(r) = tree { r } else { return Err(Error::make(6, "Root of tree should be Node::And")); }};
    if !root.anchor {
        if let Node::Chars(node_0) = &root.nodes[0] {
            if node_0.limits().min > 0 {
                let copy = node_0.string.to_string();
                match start.find(&copy) {
                    Some(offset) => {
                        if offset > 0 {
                            if trace(1) { println!("\nOptimization: RE starts with \"{}\", skipping {} bytes", node_0.string, offset); }
                            start = &start[offset..];
                            start_pos = offset;
                        }
                    },
                    None => { return Ok(None); }
                }
            };
        }
    }
    while !start.is_empty() {
        if trace(1) {println!("\n==== WALK \"{}\" ====", start)};
        let path = tree.walk(start);
        if path.len() > 1 {
            if trace(1) { println!("--- Search succeeded ---") };
            return Ok(Some((path, start_pos, char_bytes(text, start_pos))));
        }
        if trace(1) {println!("==== WALK \"{}\": no match ====", start)};
        if root.anchor { break; }
        start_pos += 1;
        start = &start[char_bytes(start, 1)..];
    }
    Ok(None)
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
// LIMITS
//
// Used to handle the number of reps allowed for a Node. Besides holding the min, max, and lazy data,
// it also handles other related questions, like whether a node falls in the allowed range, or how
// far the initial walk should go
//
//////////////////////////////////////////////////////////////////
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

    fn parse(chars: &mut Peekable) -> Result<Limits, Error> {
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

    fn parse_ints(chars: &mut Peekable) -> Result<(usize, usize), Error> {
        let num = read_int(chars);
        let peek = chars.next();
        if num.is_none() || peek.is_none(){ return Err(Error::make(10, "Unterminated repetition block")); }
        let num = num.unwrap();
        match peek.unwrap() {
            '}'=> Ok((num, num)),
            ','=> {
                let n2 = if let Some(n) = read_int(chars) { n }
                else { EFFECTIVELY_INFINITE };
                let terminate = chars.next();
                if terminate.unwrap_or('x') != '}' {Err(Error::make(11, "Malformed repetition block error 1"))}
                else {Ok((num, n2))}
            },
            _ => Err(Error::make(12, "Malformed repetition block error 2"))
        }
    }

    // Checks if the size falls in the range.
    // Returns: <0 if NUM is < min; 0 if NUM is in the range min <= NUM <= ,ax (but SEE WARNING BELOW: NUM needs
    // to be adjusted to account for the 0-match possibility.
    //
    // Beware: the input is usize and is in general the length of steps vector.
    // This has a 0-match in its first position, so the value entered is actually one higher than the allowed value.
    // I considered subtracting 1 from the number to make it match, but because the arg is usize and is sometimes 0
    // it is better to leave it like this.
    pub fn check(&self, num: usize) -> isize {
        if num <= self.min { -1 }
        else if num <= self.max + 1 { 0 }
        else { 1 }
    }

    // gives the length of the initial walk: MAX for greedy, MIN for lazy
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
struct Peekable<'a> {
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

#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub code: usize,
}

impl Error {
    fn make(code: usize, msg: &str,) -> Error { Error{code, msg: msg.to_string()}}
}

impl core::fmt::Display for Error {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Error:{}: {}", self.code, self.msg)
    }
}

//////////////////////////////////////////////////////////////////
//
// Report
//
// Used to deliver the results to the caller, a tree of results
//
//////////////////////////////////////////////////////////////////

#[derive(Debug,Clone)]
pub struct Report {
    pub found: String,
    pos: (usize, usize),
    bytes: (usize, usize),
    name: Option<String>,
    pub subreports: Vec<Report>,
}

impl Report {
    pub fn new(root: &crate::regexp::walk::Path, char_start: usize, byte_start: usize) -> Report {
        let (reports, _char_end)  = root.gather_reports(char_start, byte_start);
        reports[0].clone()
    }
    
    pub fn display(&self, indent: isize) {
        let name_str = { if let Some(name) = &self.name { format!("<{}>", name) } else { "".to_string() }};
        println!("{}\"{}\" char position [{}, {}] byte position [{}, {}] {}",
                 pad(indent), self.found, self.pos.0, self.pos.1, self.bytes.0, self.bytes.1, name_str);
        self.subreports.iter().for_each(move |r| r.display(indent + 1));
    }

    pub fn get_by_name<'b>(&'b self, name: &'b str) -> Vec<&Report> {
        let mut v = Vec::<&Report>::new();
        if let Some(n) = &self.name {
            if n == name { v.push(self); }
        }
        for r in &self.subreports {
            let mut x = r.get_by_name(name);
            v.append(&mut x);
        }
        v
    }

    pub fn get_named(& self) -> HashMap<&str, Vec<&Report>> {
        let hash = HashMap::new();
        self.get_named_internal(hash)
    }
    
    fn get_named_internal<'b>(&'b self, mut hash: HashMap<&'b str, Vec<&'b Report>>) -> HashMap<&'b str, Vec<&Report>> {
        if let Some(name) = &self.name {
            if let Some(mut_v) = hash.get_mut(&name.as_str()) {
                mut_v.push(self);
            } else {
                hash.insert(name.as_str(), vec![self]);
            }
            for r in self.subreports.iter() {
                hash = r.get_named_internal(hash);
            }
        }
        hash
    }
}
