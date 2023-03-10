use std::str::Chars;
#[cfg(test)]
mod tests;

const PEEKED_SANITY_SIZE: usize = 20;           // sanity check: peeked stack should not grow large
const EFFECTIVELY_INFINITE: usize = 99999999;   // big number to server as a cap for *
const TAB_INDENT:usize = 2;                     // indent in Debug display

#[derive(PartialEq)]
pub enum Node {Chars(CharsNode), SpecialChar(SpecialCharNode), And(AndNode), Or(OrNode), Matching(MatchingNode), Success, None, }
use core::fmt::Debug;
impl Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", match self {
            Node::Success => "Success".to_string(),
            Node::None => "None".to_string(),
            _ => self.tree_node().unwrap().desc(0)
        })
    }
}

const NODE_CHARS:    usize = 0;
const NODE_SPEC:     usize = 1;
const NODE_AND:      usize = 2;
const NODE_OR:       usize = 3;
const NODE_MATCHING: usize = 4;
const NODE_SUCCESS:  usize = 5;
const NODE_NONE:     usize = 6;

impl Node {
    fn is_none(&self) -> bool { self.node_type() == NODE_NONE }
    fn node_type(&self) -> usize{
        match self {
            Node::Chars(_a)       => NODE_CHARS,
            Node::SpecialChar(_a) => NODE_SPEC,
            Node::And(_a)         => NODE_AND,
            Node::Or(_a)          => NODE_OR,
            Node::Matching(_a)    => NODE_MATCHING,
            Node::Success         => NODE_SUCCESS,
            Node::None            => NODE_NONE,
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
        let mut prev = nodes.pop().unwrap();
        if prev.node_type() == NODE_CHARS {
            let chars_node = prev.chars_mut_ref();
            if chars_node.string.len() > 1 {
                let new_node = Node::Chars(CharsNode {string: chars_node.string.pop().unwrap().to_string(), limit_desc: (1, 1, false)});
                nodes.push(prev);
                nodes.push(new_node);
            }
        } else {
            nodes.push(prev);
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
        // TODO: check for empty
        Node::chars_before_or(nodes);
        let mut prev = nodes.pop().unwrap();
        match prev.node_type() {
            NODE_OR => {
                let prev_node = prev.or_mut_ref();
                prev_node.push(self);
                nodes.push(prev);
            },
            _ => {
                let or_node = self.or_mut_ref();
                or_node.push_front(prev);
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
            Node::Matching(a)    => Some(a),
            Node::Success        => None,
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
#[derive(PartialEq, Default)]
pub struct CharsNode {
    limit_desc: (usize, usize, bool),
    string: String,
}

// handles special characters like ".", \N, etc.
#[derive(PartialEq, Default)]
pub struct SpecialCharNode {
    limit_desc: (usize, usize, bool),
    special: char,
}

// handles AND (sequential) matches
#[derive(PartialEq, Default)]
pub struct AndNode {
    limit_desc: (usize, usize, bool),
    nodes: Vec<Node>,
}

// handles A\|B style matches
#[derive(PartialEq, Default)]
pub struct OrNode {
    // No reps for OR node, to repeat it has to be surrounded by an AND
    // limit_desc: (usize, usize, bool),
    nodes: Vec<Node>,
}

// handles [a-z] style matches
#[derive(PartialEq, Default)]
pub struct MatchingNode {
    limit_desc: (usize, usize, bool),
    targets: Vec<Matching>,
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
    fn limits(&self) -> (usize, usize, bool);
    fn desc(&self, indent: usize) -> String;
    fn limit_str(&self) -> String {
        let limits = self.limits();
        let code = 
            if limits.0 == 0 {
                if limits.1 == 1 { "?".to_string() }
                else if limits.1 >= EFFECTIVELY_INFINITE { "*".to_string() }
                else { format!("{{0,{}}}", limits.1) }
            } else if limits.0 == 1 {
                if limits.1 >= EFFECTIVELY_INFINITE { "+".to_string() }
                else if limits.1 == 1 { "".to_string() }
                else { format!("{{1,{}}}", limits.1) }
            } else if limits.0 == limits.1 {
                format!("{{{}}}", limits.0)
            } else if limits.1 < EFFECTIVELY_INFINITE {
                format!("{{{},{}}}", limits.0, limits.1)
            } else {
                format!("{{{},}}", limits.0)
            };
        format!("{}{}", code, if limits.2 {" lazy" } else { "" })
    }
}

// CharsNode keeps a string of consecuive characters to look for. Really, it is like an AndNode where each node has a single
// character, but that seems wasteful/ he sring contains no special chars, and it can have no repeat count unless there is
// only a single character.
impl TreeNode for CharsNode {
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String { format!("{}CharsNode: '{}'{}", pad(indent), self.string, self.limit_str())}
}

impl TreeNode for SpecialCharNode {
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }

    fn desc(&self, indent: usize) -> String {
        let slash = if self.special == '.' { "" } else { "\\" };
        format!("{}SpecialCharNode: '{}{}'", pad(indent), slash, self.special)
    }
}

    
// format the limis for debugging
impl TreeNode for AndNode {
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}AndNode {}", pad(indent), self.limit_str());
        for i in 0..self.nodes.len() {
            let disp_str = match self.nodes[i].tree_node() {
                Some(node) => node.desc(indent + 1),
                None => format!("{:?}", self.nodes[i]),
            };
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
}

impl TreeNode for OrNode {
    fn limits(&self) -> (usize, usize, bool) { (1, 1, false) }
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}OrNode", pad(indent), );
        for i in 0..self.nodes.len() {
            let disp_str = match self.nodes[i].tree_node() {
                Some(node) => node.desc(indent + 1),
                None => format!("{:?}", self.nodes[i]),
            };
            msg.push_str(format!("\n{}", disp_str).as_str());
        }
        msg
    }
}

impl TreeNode for MatchingNode {
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let target_str = self.targets.iter().map(|x| x.desc()).collect::<Vec<_>>().join("");
        let not = if self.not { "^" } else { "" };
        format!("{}MatchingNode [{}{}]{}", pad(indent), not, target_str, self.limit_str(), )
    }
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
    const ESCAPE_CODES: &str = "SN()n|";

    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut chs = Vec::<char>::new();
        loop {
            match chars.peek_2() {
                (Some(ch0), Some(ch1)) => {
                    if ch0 == '.' || ch0 == '[' { break; }
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
                (Some(_ch0), None) => { return Err("Bad escape char".to_string()); },
                _ => { break; }
            }
            chs.push(chars.next().unwrap());
        }
        Ok(if chs.is_empty() { Node::None }
           else {
               let limit_desc = if chs.len() == 1 { reps(chars)? } else { (1, 1, false) };
               Node::Chars(CharsNode {
                   string: chs.into_iter().collect(),
                   limit_desc
               })
           })
    }
}    

impl SpecialCharNode {
    // called with pointer at a special character. Char can be '.' or "\*". For now I'm assuming this only gets called with special
    // sequences at bat, so no checking is done.
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let special = if let Some(ch) = chars.next() {
            if ch == '.' { '.' }
            else if let Some(_ch1) = chars.peek() { chars.next().unwrap() }   // ch1 is next(), doing it this way gets the same value and pops it off
            else { return Ok(Node::None); }
        } else { return Ok(Node::None); };
        Ok(Node::SpecialChar(SpecialCharNode { special, limit_desc: reps(chars)?}))
    }
}

impl AndNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }

    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
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
           else { Node::And(AndNode {nodes, limit_desc: reps(chars)?})})
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
        Ok( Node::Or(OrNode {nodes,}))
    }
}

impl MatchingNode {
    fn push(&mut self, matching: Matching) { self.targets.push(matching); }
    
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut targets = Vec::<Matching>::new();
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
                    let target = Matching::parse_next(chars)?; 
                    if target != Matching::Empty {targets.push(target) }
                },
            }
        }
        
        // eiher None or ']'
        if let Some(ch) = chars.next() { if ch != ']' {return Err("Unterminated Matching".to_string()); } };
        Ok(if targets.is_empty() { Node::None }
           else { Node::Matching( MatchingNode {targets, not, limit_desc: reps(chars)?}) })
    }
    
}
        
// used to mark ranges for MatchingNode
// TODO: maybe combine single chars into String so can use contains() to ge all at once?
#[derive(PartialEq, Debug)]
enum Matching {RegularChars(String), SpecialChar(char), Range(char, char), Empty}

/*
impl Debug for Matching {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Matching::Range(ch0, ch1) => write!(f, "[{} - {}]", ch0, ch1),
            Matching::RegularChars(string) => write!(f, "{}", string),
            Matching::SpecialChar(ch) => write!(f, "\\{}", ch),
        }
    }
}
*/
impl Matching {
    fn desc(&self) -> String {
        match self {
            Matching::RegularChars(string) => string.to_string(),
            Matching::SpecialChar(ch) => format!("\\{}", ch),
            Matching::Range(ch0, ch1) => format!("{}-{}", ch0, ch1),
            Matching::Empty => "*EMPTY*".to_string(),
        }
    }
    fn parse_next(chars: &mut Peekable) -> Result<Matching, String> {
        Ok(match chars.peek_2() {
            (Some(ch0), Some(ch1)) => {
                if ch1 == '-'{
                    Matching::Range(chars.next().unwrap(), (chars.next(), chars.next().unwrap()).1)
                } else if ch0 == '\\' {
                    Matching::SpecialChar((chars.next(), chars.next()).1.unwrap())
                } else {
                    let mut string = "".to_string();
                    loop {
                        match chars.peek_2() {
                            (Some(ch0), Some(ch1)) => {
                                if ch0 == ']' || ch0 == '\\' || ch1 == '-' { break; }
                                string.push(chars.next().unwrap());
                            },
                            _ => { return Err("Unterminated matching block".to_string()); },
                        }
                    }
                    if string.is_empty() {Matching::Empty} else {Matching::RegularChars(string)}
                }
            }
            _ => Matching::Empty,
        })
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
    Ok(
        if ch0 == '[' {
            let _ = chars.next();
            MatchingNode::parse_node(chars)
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
        } else if ch0 == '.' {
            SpecialCharNode::parse_node(chars)
        } else {
            CharsNode::parse_node(chars)
        }?)
}

//
// Main entry point for parsing tree
//
// Wraps the in put with "\(...\)" so it becomes an AND node, and sticks the SUCCESS node on the end when done
pub fn parse_tree(input: &str) -> Result<Node, String> {
    // wrap the string in "\(...\)" to make it an implicit AND node
    let mut chars = Peekable::new(input);
    chars.push('\\');
    chars.push(')');
    let mut outer_and = AndNode::parse_node(&mut chars)?;
    let and_node = outer_and.and_mut_ref();
    and_node.push(Node::Success);
    Ok(outer_and)
}

//////////////////////////////////////////////////////////////////
//
// walk
//
// Main functions to walk the regexp tree to find a match
//
//////////////////////////////////////////////////////////////////
#[derive(Debug)]
pub struct Match {

}

pub fn walk_tree(_tree: Node, _text: &str) -> Option<Match> {
    None
}
//////////////////////////////////////////////////////////////////
//
// Helper functions
//
//////////////////////////////////////////////////////////////////

//
// These functions parse the reps option from the re source
//
fn reps(chars: &mut Peekable) -> Result<(usize, usize, bool), String> {
    let next = chars.next();
    if next.is_none() { return Ok((1, 1, false)); }
    let next = next.unwrap();
    let (min, max): (usize, usize) = match next {
        '*' => (0, EFFECTIVELY_INFINITE),
        '+' => (1, EFFECTIVELY_INFINITE),
        '?' => (0, 1),
        '{' => custom_rep(chars)?,
        _ => { chars.put_back(next); return Ok((1, 1, false)) },
    };
    let qmark = chars.peek();
    let lazy = qmark.unwrap_or('x') == '?';
    if lazy { let _ = chars.next(); }
    Ok((min, max, lazy))
}

fn custom_rep(chars: &mut Peekable) -> Result<(usize, usize), String> {
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
            if terminate.unwrap_or('x') != '}' { return Err("Malformed repetition block error 1".to_string()); }
            Ok((num, n2))
        },
        _ => Err("Malformed repetition block error 2".to_string())
    }
}

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

// helper function to format debug
fn pad(x: usize) -> String {
    let pad = TAB_INDENT*x;
    format!("{:pad$}", "")
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

