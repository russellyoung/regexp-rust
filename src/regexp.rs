use std::str::Chars;

// For now I am going to ignore unicode and deal with ascii. I'm not sure how unicode affects regexps - for instance,
// I imagine RANGE is undefined. Once this is working I will add unicode support - strings can be accessed via
// the cars() iter

const PEEKED_SANITY_SIZE: usize = 20;           // sanity check: peeked stack should not grow large
const EFFECTIVELY_INFINITE: usize = 99999999;   // big number to server as a cap for *
const TAB_INDENT:usize = 4;                     // indent in Debug display

// This is an iterator with 2 added features:
//     1) peeking: the next char can be peeked (read without consuming) or returned after being consumed
//     2) extra characters 
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
    
    pub fn trail_push(&mut self, ch: char) { self.trailer.push(ch); }


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

// so the nodes can report their type
#[derive(PartialEq)]
enum NodeType {Chars, Special, And, Or, Has, Hasnt, Success}

pub trait Node {
    fn node_type(&self) -> NodeType;
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String>;
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
            } else if limit.0 < EFFECTIVELY_INFINITE {
                format!("{{{},{}}}", limit.0, limit.1)
            } else {
                format!("{{{},}}", limit.0)
            };
        format!("{}{}", code, if limit.2 {" lazy" } else { "" })
    }
}

use core::fmt::Debug;
impl Debug for dyn Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.desc(0))
    }
}

// handles strings of regular characters
#[derive(Debug, Default, )]
struct CharsNode {
    limit_desc: (usize, usize, bool),
    string: String,
}

// handles special characters like ".", \N, etc.
#[derive(Debug, Default, )]
struct SpecialCharNode {
    limit_desc: (usize, usize, bool),
    special: char,
}

// handles AND (sequential) matches
#[derive(Debug, Default, )]
struct AndNode {
    limit_desc: (usize, usize, bool),
    nodes: Vec<Box<dyn Node>>,
}

// handles A\|B style matches
#[derive(Debug, Default, )]
struct OrNode {
    limit_desc: (usize, usize, bool),
    nodes: Vec<Box<dyn Node>>,
}

// used to mark ranges for HasNode and HasntNode
enum Range {Single(char), Range(char, char)}
impl Debug for Range {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Range::Range(x, y) => write!(f, "[{} - {}]", x, y),
            Range::Single(x) => write!(f, "{}", x),
        }
    }
}

impl Range {
    fn desc(&self) -> String {
        match self {
            Range::Single(x) => format!("{}", x),
            Range::Range(x, y) => format!("{}-{}", x, y),
        }
    }
}

// handles [a-z] style matches
#[derive(Debug, Default, )]
struct HasNode {
    limit_desc: (usize, usize, bool),
    targets: Vec<Range>,
}

// handles [^a-z] style matches
#[derive(Debug, Default, )]
struct HasntNode {
    limit_desc: (usize, usize, bool),
    targets: Vec<Range>,
}

// signals the end of the tree in phase 2, tree-walking
#[derive(Debug, Default, )]
struct Success {
}

// These characters are have meaning when escaped, break for them. Otherwise just delete the '/' from the string
const ESCAPE_CODES: &str = "HN()n";

impl Node for CharsNode {
    fn node_type(&self) -> NodeType { NodeType::Chars }
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    
    fn desc(&self, indent: usize) -> String { format!("{}CharsNode: '{}'{}", pad(indent*TAB_INDENT), self.string, self.limit_str())}
    
    // CharsNode keeps a string of consecuive characters to look for. Really, it is like an AndNode where each node has a single
    // character, but that seems wasteful/ he sring contains no special chars, and it can have no repeat count unless there is
    // only a single character.
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> {
        let mut chs = Vec::<char>::new();
        loop {
            let ch = chars.peek();
            if ch.is_none() { break; }
            let ch = ch.unwrap();
            if ch == '\\' {
                let ch1 = chars.peek();
                if ch1.is_none() { return Err("Bad escape char".to_string()); }
                let ch1 = ch1.unwrap();
                if ESCAPE_CODES.contains(ch1) {
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
        if chs.len() > 0 {
            self.limit_desc = if chs.len() == 1 { reps(chars)? } else { (1, 1, false) };
            self.string = chs.into_iter().collect();
        }
        Ok(self.string.len() > 0)
    }
}

impl Node for SpecialCharNode {
    fn node_type(&self) -> NodeType { NodeType::Special }
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }

    fn desc(&self, indent: usize) -> String {
        let slash = if self.special == '.' { "" } else { "\\" };
        format!("{}SpecialCharNode: '{}{}'", pad(indent*TAB_INDENT), slash, self.special)
    }

    // called with pointer at a special character. Char can be '.' or "\*". For now I'm assuming this only gets called with special
    // sequences at bat, so no checking is done.
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> {
        let ch = chars.peek();
        if ch.is_none() { return Ok(false); }
        let ch = ch.unwrap();
        if ch != '.' { let _ = chars.next(); }
        self.special = chars.next().unwrap();     // TODO: None
        let limit_result = reps(chars);
        if limit_result.is_err() { return Err(limit_result.err().unwrap()); }
        self.limit_desc = limit_result.unwrap();
        Ok(true)
    }
}

// format the limis for debugging
impl Node for AndNode {
    fn node_type(&self) -> NodeType { NodeType::And }
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}AndNode {}", pad(indent), self.limit_str());
        for i in 0..self.nodes.len() {
            msg.push_str(format!("\n{}", self.nodes[i].desc(indent + 1)).as_str());
        }
        msg
    }
    
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> {
        loop {
            let (ch0, ch1) = chars.peek_2();
            if ch0.is_none() { return Err("Unterminated AND node".to_string()); }
            if ch0.unwrap() == '\\' && ch1.unwrap_or('x') == ')' { break; }
            let node = parse(chars)?;
            if node.is_some() {
                let node = node.unwrap();
                // OR is tricky: if the preceeding node in the AND is an OR the OrNode gets tossed and its
                // contents gets added to the previous OR. If the previous node is anything else it it is
                // moved to the first position in the OrNode and the OrNode replaces it in the AndNode lis
                if node.node_type() == NodeType::Or {
                    self.handle_or(node);
                } else {
                    self.nodes.push(node);
                }
            }
        }
        // pop off terminating chars
        let _ = chars.next();
        let _ = chars.next();
        self.limit_desc = reps(chars)?;
        return Ok(self.nodes.len() > 0);
    }
}

impl AndNode {
    fn push(&mut self, node: Box<dyn Node>) { self.nodes.push(node); }
    fn handle_or(&mut self, node: Box<dyn Node>) {
        /*
        // TODO: empty vec
        let prev = self.nodes.get_mur(self.nodes.len() - 1).unwrap();
        if prev.node_type() == NodeType::Or {
        prev.push(node
         */
    }
}

impl Node for OrNode {
    fn node_type(&self) -> NodeType { NodeType::Or }
    fn limits(&self) -> (usize, usize, bool) { (1, 1, false) }
    fn desc(&self, indent: usize) -> String {
        let mut msg = format!("{}OrNode {}", pad(indent), self.limit_str());
        for i in 0..self.nodes.len() {
            msg.push_str(format!("\n{}", self.nodes[i].desc(indent + 1)).as_str());
        }
        msg
    }
    
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> {
        let node = parse(chars)?;
        if !node.is_some() { Ok(false) }
        else {
            self.nodes.push(node.unwrap());
            Ok(true)
        }
    }
}

impl OrNode {
    fn push(&mut self, node: Box<dyn Node>) { self.nodes.push(node); }
}

impl Node for HasNode {
    fn node_type(&self) -> NodeType { NodeType::Has }
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> { Err("todo".to_string()) }
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let target_str = self.targets.iter().map(|x| x.desc()).collect::<Vec<_>>().join("");
        format!("{}HasNode [{}]{}", pad(indent), target_str, self.limit_str(), )
    }
}

impl Node for HasntNode {
    fn node_type(&self) -> NodeType { NodeType::Hasnt }
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> { Err("todo".to_string()) }
    fn limits(&self) -> (usize, usize, bool) { self.limit_desc }
    fn desc(&self, indent: usize) -> String {
        let target_str = self.targets.iter().map(|x| x.desc()).collect::<Vec<_>>().join("");
        format!("{}HasntNode [{}]{}", pad(indent), target_str, self.limit_str(), )
    }
}

impl Node for Success {
    fn node_type(&self) -> NodeType { NodeType::Success }
    fn parse(&mut self, chars: &mut Peekable) -> Result<bool, String> { Err("todo".to_string()) }
    fn limits(&self) -> (usize, usize, bool) { (0, 0, false, )}
    fn desc(&self, indent: usize) -> String { format!("{}Success", pad(indent), ) }
}

impl Success {
    fn new() -> Success { Success {}}
}

fn parse(chars: &mut Peekable) -> Result<Option<Box<dyn Node>>, String> {
    let (ch0, ch1) = chars.peek_2();
    if ch0.is_none() { return Ok(None); }
    let ch0 = ch0.unwrap();
    let ch1 = ch1.unwrap_or(' ');  // SPACE isn't in any special character sequence
    let mut node:Box<dyn Node> = 
        if ch0 == '[' {
            let _ = chars.next();
            if ch1 == '^' {
                let _ = chars.next();
                Box::new(HasntNode::default())
            } else {
                Box::new(HasNode::default())
            }
        } else if ch0 == '\\' {
            if ch1 != '(' && ch1 != '|' {
                if ch1 == '\\' {
                    Box::new(CharsNode::default())
                } else {
                    Box::new(SpecialCharNode::default())
                }
            } else {
                let _ = chars.next();
                let _ = chars.next();
                if ch1 == '(' {
                    Box::new(AndNode::default())
                } else {
                    Box::new(OrNode::default())
                }
            }
        } else {
            Box::new(CharsNode::default())
        };
    Ok( if node.parse(chars)? { Some(node) } else { None })
}

//
// Main entry point for parsing tree
//
pub fn parse_tree(input: &str) -> Result<Box<dyn Node>, String> {
    // wrap the string in "\(...\)" to make it an implicit AND node
    let mut string = input.to_owned();
    string.push_str(r"\)");
    let mut chars = Peekable::new(&string);

    let mut tree = AndNode::default();
    let _ = tree.parse(&mut chars)?;
    tree.push(Box::new(Success::new()));
    Ok(Box::new(tree))
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
const BLANKS: &str = "                                                           ";
fn pad<'a>(x: usize) -> &'a str { &BLANKS[0..x] }
