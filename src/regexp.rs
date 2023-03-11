use std::str::Chars;

const PEEKED_SANITY_SIZE: usize = 20;           // sanity check: peeked stack should not grow large
const EFFECTIVELY_INFINITE: usize = 99999999;   // big number to server as a cap for *
const TAB_INDENT:usize = 4;                     // indent in Debug display

//#[derive(PartialEq)]
pub enum Node {Chars(CharsNode), SpecialChar(SpecialCharNode), And(AndNode), Or(OrNode), Range(RangeNode), Success, None, }
use core::fmt::Debug;
impl Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.tree_node().desc(0))
    }
}

const NODE_CHARS:   usize = 0;
const NODE_SPEC:    usize = 1;
const NODE_AND:     usize = 2;
const NODE_OR:      usize = 3;
const NODE_RANGE:   usize = 4;
const NODE_SUCCESS: usize = 5;
const NODE_NONE:    usize = 6;

impl Node {
    fn is_none(&self) -> bool { self.node_type() == NODE_NONE }
    fn node_type(&self) -> usize{
        match self {
            Node::Chars(_a)       => NODE_CHARS,
            Node::SpecialChar(_a) => NODE_SPEC,
            Node::And(_a)         => NODE_AND,
            Node::Or(_a)          => NODE_OR,
            Node::Range(_a)       => NODE_RANGE,
            Node::Success         => NODE_SUCCESS,
            Node::None            => NODE_NONE,
        }
    }
        
    //
    // Access Node content structs
    //
    
    // this gets a reference to the enclosed data as an immutable TreeNode. To get a ref to the object itself
    // use (or add) the methods below
    fn tree_node(&self) -> &dyn TreeNode {
        match self {
            Node::Chars(a)       => a,
            Node::SpecialChar(a) => a,
            Node::And(a)         => a,
            Node::Or(a)          => a,
            Node::Range(a)       => a,
            Node::Success        => panic!("trying to access Success node struct"),
            Node::None           => panic!("trying to access None node struct"),
        }
    }
    // following are methods to access the different types by ref, muable and immutable. I wonder if it is possible
    // for a single templae to handle all cases?
    fn or_ref(&self) -> &OrNode {
        match self {
            Node::Or(node) => &node,
            _ => panic!("Attempting to get ref to OrNode from wrong Node type")
        }
    }
    // thank you rust-lang.org
    fn or_mut_ref(&mut self)->Option<&mut OrNode> {
        match self {
            Node::Or(or_node) => Some(or_node),
            _ => panic!("Attempting to get mut ref to OrNode from wrong Node type")
        }
    }
    fn and_mut_ref(&mut self)->Option<&mut AndNode> {
        match self {
            Node::And(and_node) => Some(and_node),
            _ => panic!("Attempting to get mut ref to AndNode from wrong Node type")
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
#[derive(Default)]
pub struct CharsNode {
    limit_desc: (usize, usize, bool),
    string: String,
}

// handles special characters like ".", \N, etc.
#[derive(Default)]
pub struct SpecialCharNode {
    limit_desc: (usize, usize, bool),
    special: char,
}

// handles AND (sequential) matches
#[derive(Default)]
pub struct AndNode {
    limit_desc: (usize, usize, bool),
    nodes: Vec<Node>,
}

// handles A\|B style matches
#[derive(Default)]
pub struct OrNode {
    // No reps for OR node, to repeat it has to be surrounded by an AND
    // limit_desc: (usize, usize, bool),
    nodes: Vec<Node>,
}

// handles [a-z] style matches
#[derive(Default)]
pub struct RangeNode {
    limit_desc: (usize, usize, bool),
    targets: Vec<Range>,
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
        let limit = self.limits();
        let code = 
            if limit.0 == 0 {
                if limit.1 == 1 { "?".to_string() }
                else if limit.1 >= EFFECTIVELY_INFINITE { "*".to_string() }
                else { format!("{{0,{}}}", limit.1) }
            } else if limit.0 == 1 {
                if limit.1 >= EFFECTIVELY_INFINITE { "+".to_string() }
                else if limit.1 == 1 { "".to_string() }
                else { format!("{{1,{}}}", limit.1) }
            } else if limit.0 == limit.1 {
                format!("{{{}}}", limit.0)
            } else if limit.1 < EFFECTIVELY_INFINITE {
                format!("{{{},{}}}", limit.0, limit.1)
            } else {
                format!("{{{},}}", limit.0)
            };
        format!("{}{}", code, if limit.2 {" lazy" } else { "" })
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
            msg.push_str(format!("\n{}", self.nodes[i].tree_node().desc(indent + 1)).as_str());
        }
        msg
    }
    
}

impl TreeNode for OrNode {
    fn limits(&self) -> (usize, usize, bool) { (1, 1, false) }
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}OrNode", pad(indent), );
        for i in 0..self.nodes.len() {
            msg.push_str(format!("\n{}", self.nodes[i].tree_node().desc(indent + 1)).as_str());
        }
        msg
    }
}

impl TreeNode for RangeNode {
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let target_str = self.targets.iter().map(|x| x.desc()).collect::<Vec<_>>().join("");
        let not = if self.not { "^" } else { "" };
        format!("{}RangeNode [{}{}]{}", pad(indent), not, target_str, self.limit_str(), )
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
            let ch = chars.peek();
            if ch.is_none() { break; }
            let ch = ch.unwrap();
            if ch == '\\' {
                let ch1 = chars.peek();
                if ch1.is_none() { return Err("Bad escape char".to_string()); }
                let ch1 = ch1.unwrap();
                if CharsNode::ESCAPE_CODES.contains(ch1) {
                    break;     // this is an escape character, break and use the predecing characters
                }
                let _ = chars.next();      // eat the "\" so the trailing character is put in the string
            } else if ch == '.' { break; } // another special character, use preceeding in string and leave this for next Node
            else if "?*+{".contains(ch) {  // it is a rep count - this cannot apply to a whole string, just a single character
                chars.put_back(chs.pop().unwrap());   // TODO: can chs be empty?
                break;
            }
            chs.push(chars.next().unwrap());
        }
        Ok(if chs.len() == 0 { Node::None }
           else {
               let limit_desc = if chs.len() == 1 { reps(chars)? } else { (1, 1, false) };
               Node::Chars(CharsNode {
                   string: chs.into_iter().collect(),
                   limit_desc: limit_desc
               })
           })
    }
}    

impl SpecialCharNode {
    // called with pointer at a special character. Char can be '.' or "\*". For now I'm assuming this only gets called with special
    // sequences at bat, so no checking is done.
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let ch = chars.peek();
        if ch.is_none() { return Ok(Node::None); }
        let ch = ch.unwrap();
        if ch != '.' { let _ = chars.next(); }
        let special = chars.next().unwrap();     // TODO: None
        Ok(Node::SpecialChar(SpecialCharNode { special: special, limit_desc: reps(chars)?}))
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
            if node.node_type() != NODE_NONE {
                if node.node_type() == NODE_OR {
                    AndNode::handle_or(&mut nodes, node);
                } else {
                    nodes.push(node);
                }
            }
        }
        // pop off terminating chars
        let _ = chars.next();
        let _ = chars.next();
        Ok(if nodes.len() == 0 { Node::None }
           else { Node::And(AndNode {nodes: nodes, limit_desc: reps(chars)?})})
    }
    
    // OR is tricky: if the preceeding node in the AND is an OR the OrNode gets tossed and its
    // contents gets added to the previous OR. If the previous node is anything else it it is
    // moved to the first position in the OrNode and the OrNode replaces it in the AndNode list
    fn handle_or(nodes: &mut Vec<Node>, mut node: Node) {
        // TODO: check for empty
        let mut prev = nodes.pop().unwrap();
        if prev.node_type() == NODE_OR {
            let prev_node = prev.or_mut_ref().unwrap();
            prev_node.push(nodes.pop().unwrap());
            nodes.push(prev);
        } else {
            let or_node = node.or_mut_ref().unwrap();
            or_node.push(prev);
            nodes.push(node);
        }
    }
}

impl OrNode {
    fn push(&mut self, node: Node) { self.nodes.push(node); }
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut nodes = Vec::<Node>::new();
        let node = parse(chars)?;
        if !node.is_none() {
            nodes.push(node);
        }
        Ok( Node::Or(OrNode {nodes: nodes,}))
    }
}

impl RangeNode {
    fn parse_node(chars: &mut Peekable) -> Result<Node, String> {
        let mut terminated = false;
        let mut targets = Vec::<Range>::new();
        let mut not = false;
        let ch0 = chars.peek();
        if ch0.is_some() {
            let ch0 = ch0.unwrap();
            if ch0 == '^' {
                let _ = chars.next();
                not = true;
            }
        }
        loop {
            let ch = chars.peek();
            if ch.is_none() { break; }
            let ch = ch.unwrap();
            if ch == ']' {
                terminated = true;
                break;
            }
            targets.push(RangeNode::parse_next(chars));
        }
        if !terminated { return Err("Unterminated Range".to_string()); }
        Ok(if targets.len() == 0 { Node::None }
           else { Node::Range( RangeNode {targets: targets, not: not, limit_desc: reps(chars)?}) })
    }
    
    fn parse_next(chars: &mut Peekable) -> Range {
        let ch0 = chars.next().unwrap();     // empty case is handled in caller
        let ch1 = chars.peek().unwrap_or('x');
        if ch0 == '\\' {
            let _ = chars.next();
            Range::SpecialChar(ch1)
        } else if ch1 == '-' {
            let _ = chars.next();
            let ch2 = chars.next();
            if ch2.is_none() { Range::SingleChar('-') }
            else {Range::Range(ch0, ch2.unwrap()) }
        } else {
            Range::SingleChar(ch0)
        }
    }    
}
    
// used to mark ranges for RangeNode
// TODO: maybe combine single chars into String so can use contains() to ge all at once?
enum Range {SingleChar(char), SpecialChar(char), Range(char, char)}
/*
impl Debug for Range {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Range::Range(x, y) => write!(f, "[{} - {}]", x, y),
            Range::SingleChar(x) => write!(f, "{}", x),
            Range::SpecialChar(x) => write!(f, "\\{}", x),
        }
    }
}
*/
impl Range {
    fn desc(&self) -> String {
        match self {
            Range::SingleChar(x) => format!("{}", x),
            Range::SpecialChar(x) => format!("\\{}", x),
            Range::Range(x, y) => format!("{}-{}", x, y),
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
    Ok(
        if ch0 == '[' {
            let _ = chars.next();
            RangeNode::parse_node(chars)
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
        } else {
            CharsNode::parse_node(chars)
        }?)
}

//
// Main entry point for parsing tree
//
// Wraps the in put with "\(...\)" so it becomes an AND node, and sticks the SUCCESS node on the end when done
pub fn parse_tree(input: &mut String) -> Result<Node, String> {
    // wrap the string in "\(...\)" to make it an implicit AND node
    let mut chars = Peekable::new(input);
    chars.push('\\');
    chars.push(')');
    let mut outer_and = AndNode::parse_node(&mut chars)?;
    let and_node = outer_and.and_mut_ref().unwrap();
    and_node.push(Node::Success);
    Ok(outer_and)
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
    let (min, max): (usize, usize);
    let next = chars.next();
    if next.is_none() { return Ok((1, 1, false)); }
    let next = next.unwrap();
    match next {
        '*' => (min, max) = (0, EFFECTIVELY_INFINITE),
        '+' => (min, max) = (1, EFFECTIVELY_INFINITE),
        '?' => (min, max) = (0, 1),
        '{' => (min, max) = custom_rep(chars)?,
        _ => { chars.put_back(next); return Ok((1, 1, false)) },
    }
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
            let n2 = read_int(chars);
            let n2 = if n2.is_none() {EFFECTIVELY_INFINITE} else {n2.unwrap()};
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
        if digit < '0' || digit > '9' {
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
pub struct Peekable<'a> {
    chars: Chars<'a>,
    peeked: Vec<char>,
    trailer: Vec<char>,
    progress_check: isize,
}

impl<'a> Peekable<'a> {
    fn new(string: &String) -> Peekable { Peekable { chars: string.chars(), peeked: Vec::<char>::new(), trailer: Vec::<char>::new(), progress_check: 1} }

    pub fn next(&mut self) -> Option<char> {
        if !self.peeked.is_empty() { Some(self.peeked.remove(0)) }
        else { self.next_i() }
    }

    // peek() looks at the next character in the pipeline. If called multiple times it returns the same value
    pub fn peek(&mut self) -> Option<char> {
        if self.peeked.is_empty() {
            let ch = self.next_i();
            if ch.is_none() { return None; }
            self.peeked.push(ch.unwrap());
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
            ret = if self.trailer.len() > 0 { Some(self.trailer.remove(0)) } else { None };
        }
        self.progress_check += 1;
        ret
    }
            
    // peek_next() gets the next unread character, adds it to the peeked list, and returns it
    fn peek_next(&mut self) -> Option<char> {
        let ch = self.next_i();
        if ch.is_some() {
            self.peeked.push(ch.unwrap());
        }
        ch
    }

}

