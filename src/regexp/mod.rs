//! ## Regular expression search: RE parser
//! This module offers all functionality for RE searches. It contains the code to parse the RE into a tree, and also exports
//! the functionality to walk the tree and display the results.
//! 
//! The REs are parsed with the function *parse_tree()*, which returns a tree (actually an **AndNode**, which is the root of the tree)
//! which can then be used in the walk phase to look for matches.
// TODO:
// - refactor: especially in WALK there seems to be a lot of repeat code. Using traits I think a lot can be consolidated
pub mod walk;
#[cfg(test)]
mod tests;

use std::str::Chars;
use crate::{trace, trace_indent, trace_change_indent, trace_set_indent, TAB_SIZE};
use crate::regexp::walk::Matched;
use core::fmt::{Debug,};
use std::collections::HashMap;
use home;

/// big number to server as a cap for repetition count
const EFFECTIVELY_INFINITE: usize = 99999999;

//////////////////////////////////////////////////////////////////
//
// Node
//
// Nodes act as a container to hold the TreeNodes that make up the tree. At first I used Box for everything but that
// made it hard to keep track of what was what, eventually I thought if using enums as wrapper. That makes passing
// things around conveneint, though it does require some way of getting back the TreeNode object. At first I tried
// making a trait to hold all the TreeNode structs, having all Node types hold 'dyn TreeNode' but it turned out in
// this case not to be too useful - there were enough differences in the TreeNodes that it was not really natural
// to generalize all the methods needed, or at least I could not see such good definitions, so instead common
// functionality is provided through methods on the Node enum. ALso, using 'dyn TreeNode' did not seem to work
// because I got error messages that the size was unknown at compile time. I don't know if I could have worked
// aound this or not, but in any case it turned out using Node methods worked well.
//
// I also considered using a union for the contents - that is another way besides trait of having the same contents
// type for everything. I think that would work well, but in the end I don't see a big advantage over using enums,
// and I get the feeling unions are not as much a mainstream feature. Also, they require using 'unsafe', which it
// seems best to avoid whenever possible.
//
//////////////////////////////////////////////////////////////////

/// Node acts as a common wrapper for the different XNode struct types: CharsNode, SpecialCharNode, SetNode, AndNode, and OrNode.
/// Besides serving as a common strcut to distribute message requests, it also behaves like Box in providing a place in memory for the structures to live.
#[derive(PartialEq)]
pub enum Node {Chars(CharsNode), And(AndNode), Or(OrNode), Range(RangeNode), Special(SpecialNode), Def(DefNode), None, }

impl core::fmt::Debug for Node {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Node::Chars(a) => a.fmt(f),
            Node::Special(a) => a.fmt(f),
            Node::Range(a) => a.fmt(f),
            Node::And(a) => a.fmt(f),
            Node::Or(a) => a.fmt(f),
            Node::Def(a) => a.fmt(f),
            Node::None => write!(f, "None")
        }
    }
}

impl Clone for Node {
    fn clone(&self) -> Node {
        match self {
            Node::Chars(chars_node) => Node::Chars(chars_node.clone()),
            Node::Special(special_node) => Node::Special(special_node.clone()),
            Node::Range(range_node) => Node::Range(range_node.clone()),
            Node::And(and_node) => Node::And(and_node.clone()),
            Node::Or(or_node) => Node::Or(or_node.clone()),
            Node::Def(def_node) => Node::Def(def_node.clone()),
            Node::None => Node::None,
        }
    }
}    
impl Node {
    /// function that simply distributes a walk request to the proper XNode struct
    fn walk<'a>(&'a self, matched: Matched<'a>) -> walk::Path<'a> {
        match self {
            Node::Chars(chars_node) => walk::CharsStep::walk(chars_node, matched),
            Node::Special(special_node) => walk::SpecialStep::walk(special_node, matched),
            Node::Range(range_node) => walk::RangeStep::walk(range_node, matched),
            Node::And(and_node) => walk::AndStep::walk(and_node, matched),
            Node::Or(or_node) => walk::OrStep::walk(or_node, matched),
            Node::Def(def_node) => def_node.node.walk(matched),
            Node::None => panic!("NONE node should not be in final tree")
        }
    }

    /// **desc()** is like Debug or Display, but for branches it pretty-prints both the node and its descendents
    pub fn desc(&self, indent: usize) {
        match self {
            Node::Chars(a) => a.desc(indent),
            Node::Special(a) => a.desc(indent),
            Node::Range(a) => a.desc(indent),
            Node::And(a) => a.desc(indent),
            Node::Or(a) => a.desc(indent),
            Node::Def(a) => a.desc(indent),
            Node::None => print!("{0:1$}", "None", indent),
        }
    }
        
    fn set_named(&mut self, named: Option<String>) {
        match self {
            Node::Chars(a) => a.named = named,
            Node::Special(a) => a.named = named,
            Node::Range(a) => a.named = named,
            Node::And(a) => a.named = named,
            Node::Or(a) => a.named = named,
            Node::Def(a) => a.node.set_named(named),
            Node::None => panic!("No name for None node"),
        };
    }

    fn named(&self) -> &Option<String> {
        match self {
            Node::Chars(a) => &a.named,
            Node::Special(a) => &a.named,
            Node::Range(a) => &a.named,
            Node::And(a) => &a.named,
            Node::Or(a) => &a.named,
            Node::Def(a) => a.node.named(),
            Node::None => panic!("No name for None node"),
        }
    }

    fn set_limits(&mut self, limits: Limits) {
        match self {
            Node::Chars(a) => a.limits = limits,
            Node::Special(a) => a.limits = limits,
            Node::Range(a) => a.limits = limits,
            Node::And(a) => a.limits = limits,
            Node::Or(a) => a.limits = limits,
            Node::Def(a) => a.node.set_limits(limits),
            Node::None => panic!("No limits for None node"),
        };
    }

    /// checks whether the node is the special Node::None type, used to initialize structures and in case of errors. In general
    /// if the code finds this it means an error condition
    fn is_none(&self) -> bool { *self == Node::None }

    /// Called on Node creation, so if the race level is 2 or higher a message is output on node creation
    /// **Important:** This function decreases the indent depth when called from AND or OR. It should be at the same trace level as
    /// the *trace_enter()* call, which increases the indent when AND or OR are entered.
    fn trace(self) -> Self {
        if trace(2) {
            match &self {
                Node::Def(_) | Node::And(_) | Node::Or(_) => trace_change_indent(-1),
                _ => ()
            }
            trace_indent();
            println!("Created {:?}", self);
        }
        self
    }
}
/// A debugging/trace function, called when a node starts parsing the RE. It should only be called after checking he trace level.
/// **Important:** This function increases the indent depth when called from AND or OR. It should be at the same trace level as
/// the *Node::trace()* call, which reduces the indent when AND or OR are exited.
fn trace_enter(name: &str, chars: &mut Peekable) {
    trace_indent();
    println!("{} starting from \"{}\"", name, chars.preview(6));
    if name == "DEF" || name == "AND" || name == "OR" { trace_change_indent(1); }
}

//
// Node struct subtypes: these are wrapped in the Node enum to make them easy to pass around
//

/// represents strings of regular characters that match themselves in the target string. This is a leaf node in the parse tree.
/// Since character strings are implicit ANDs the limit only applies if there is a single char in the string.
#[derive(Default, PartialEq,Clone)]
pub struct CharsNode {
    limits: Limits,
    named: Option<String>,
    string: String,
}

#[derive(Default, PartialEq,Clone)]
pub struct SpecialNode {
    limits: Limits,
    named: Option<String>,
    special: char,
}

#[derive(Default,PartialEq,Clone)]
pub struct RangeNode {
    limits: Limits,
    named: Option<String>,
    not: bool,
    chars: String,
    ranges: Vec<Range>,
}


/// handles AND (sequential) matches: this node represents a branch in the parse tree
#[derive(Default, PartialEq)]
pub struct AndNode {
    limits: Limits,
    /// NAMED == None means do not report, NAMED == "" means unnamed 
    named: Option<String>,
    nodes: Vec<Node>,
    anchor: bool
}

/// handles OR nodes (A\|B style matches). This node represents a branch in the parse tree
#[derive(Default, PartialEq)]
pub struct OrNode {
    nodes: Vec<Node>,
    /// Limits for OR nodes are different from other nodes. ORs cannot be repeated (except by enclosing them in
    /// an AND), so Limits is used for OR to move through the different branches rather than the different repetitions
    limits: Limits,
    named: Option<String>,
}

// TODO: This should contain a ref to the node in the defs table, but this requires major lifeline changes which I'm
// not ready to do now, so in the short term it should have its own copy
#[derive(PartialEq)]
pub struct DefNode {
    name: String,
    node: Box<Node>,
}

impl Default for DefNode {
    fn default() -> DefNode  {DefNode { name: "".to_string(), node: Box::new(Node::None),  }  }
}

impl Clone for AndNode {
    fn clone(&self) -> AndNode {
        AndNode{limits: self.limits, named: self.named.clone(), anchor: self.anchor, nodes: self.nodes.to_vec()}
    }
}

impl Clone for OrNode {
    fn clone(&self) -> OrNode {
        OrNode{limits: self.limits, named: self.named.clone(), nodes: self.nodes.to_vec()}
    }
}
impl Clone for DefNode {
    fn clone(&self) -> DefNode {
        DefNode{node: self.node.clone(), name: self.name.clone()}
    }
}

//////////////////////////////////////////////////////////////////
//
// Node implementations
//
// The most important is that each one needs to define a contructor taking the Peekable as input
// and returning a Node enum element (complete with its TreeNode filling)
//
//////////////////////////////////////////////////////////////////
impl Debug for CharsNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match &self.named {
            Some(name) => format!("<{}>", name,),
            None => "".to_string(),
        };
        write!(f, "CharsNode{}: \"{}\"{}", name, self.string, self.limits.simple_display())
    }
}
    
impl CharsNode {
    fn parse_node(chars: &mut Peekable, after_or: bool) -> Result<Node, Error> {
        let mut node = CharsNode::default();
        let mut count = 0;
        if trace(2) { trace_enter("CHARS", chars); }
        loop {
            match chars.peek_n(3)[..] {
                [Some('\\'), Some(ch1), o_ch2] => {
                    if "()|".contains(ch1) || "*?+|".contains(o_ch2.unwrap_or('x')) { break; }
                    if SpecialNode::ESCAPE_CODES.contains(ch1) { break; }
                    node.string.push(CharsNode::escaped_chars(ch1));
                    count += 1;
                    chars.consume(2);
                },
                [Some(ch0), _, _] => {
                    if "[$.*+?{".contains(ch0) { break; }
                    count += 1;
                    node.string.push(chars.next().unwrap());
                },
                _ => { break; }
            }
            if after_or && count > 0 { break; }
        }
        match count {
            0 => { return Ok(Node::None)},
            1 => node.limits = Limits::parse(chars)?,
            _ => {
                if let (Some(ch0), Some(ch1)) = chars.peek_2() {
                    if "*?+{".contains(ch0) || (ch0 == '\\' && ch1 == '|') {
                        chars.put_back(node.string.pop().unwrap());
                    }
                }
            }
        }
        Ok(Node::Chars(node))
    }
    /// Checks a string to see if its head matches the contents of this node
    fn matches(&self, string: &str) -> Option<usize> {
        if string.starts_with(self.string.as_str()) { Some(self.string.len()) }
        else { None }
    }
    
    fn desc(&self, indent: usize) { println!("{0:1$}{2:?}", "", indent, self); }
    fn escaped_chars(ch: char) -> char {
        match ch {
            'n' => '\n',
            't' => '\t',
            c => c
        }
    }
    
    /// recovers a CharsNode from the Node::Chars enum
    fn mut_from_node(node: &mut Node) -> & mut CharsNode {
        if let Node::Chars(chars_node) = node { chars_node }
        else { panic!("trying to get CharsNode from wrong type") }
    }
}    

impl Debug for SpecialNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match &self.named {
            Some(name) => format!("<{}>", name,),
            None => "".to_string(),
        };
        let slash = if ".$".contains(self.special) { "" } else { "\\" };
        write!(f, "SpecialNode{}: \"{}{}\"{}", name, slash, self.special, self.limits.simple_display())
    }
}
    
impl SpecialNode {
    const ESCAPE_CODES: &str = "adluox";
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        let mut node = SpecialNode::default();
        if trace(2) { trace_enter("SPECIAL", chars); }
        match (chars.next(), chars.peek()) {
            (Some('\\'), _) => node.special = chars.next().unwrap(),
            (Some('.'), _) => node.special = '.',
            (Some('$'), _) => node.special = '$',
            (_, _) => panic!("Bad value passed to SpecialNode::parse_node()"),
        }
        node.limits = Limits::parse(chars)?;
        Ok(Node::Special(node))
    }

    fn matches(&self, string: &str) -> Option<usize> {
        if let Some(ch) = string.chars().next() {
            if match self.special {
                '.' => true,
                'a' => (' '..='~').contains(&ch),   // ascii printable
                'd' => ('0'..='9').contains(&ch),   // numeric
                'l' => ('a'..='z').contains(&ch),   // lc ascii
                'o' => ('0'..='7').contains(&ch),   // octal digit
                'u' => ('A'..='Z').contains(&ch),   // uc ascii
                'x' => ('A'..='F').contains(&ch)    // hex digit
                    || ('a'..='f').contains(&ch)
                    || ('0'..='9').contains(&ch),
                _ => false }
            { return Some(char_bytes(string, 1)); }
        } else if self.special == '$' { return Some(0); }
        None
    }
    fn desc(&self, indent: usize) { println!("{0:1$}{2:?}", "", indent, self); }
}


#[derive(Default,PartialEq,Clone)]
struct Range { from: char, to: char}

impl Range {
    fn contains(&self, ch: char) -> bool { self.from <= ch && ch <= self.to }
    fn to_string(&self) -> String { format!("{}-{}", self.from, self.to) }
}

impl Debug for RangeNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match &self.named {
            Some(name) => format!("<{}>", name,),
            None => "".to_string(),
        };
        write!(f, "RangeNode{}: \"{}\"{}", name, self.to_string(), self.limits.simple_display())
    }
}
    
impl RangeNode {
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        let mut node = RangeNode::default();
        if trace(2) { trace_enter("SPECIAL", chars); }
        if let Some('^') = chars.peek() {
            chars.consume(1);
            node.not = true;
        }
        loop {
            match chars.peek_n(3)[..] {
                [Some(']'), _, _] => { break; },
                [Some('\\'), Some(ch1), _] => {
                    node.chars.push(RangeNode::escapes(ch1));
                    chars.consume(2);
                },
                [Some(ch0), Some('-'), Some(']')] => {
                    node.chars.push(ch0);
                    node.chars.push('-');
                    chars.consume(2);
                },
                [Some(ch0), Some('-'), Some(ch2)] => {
                    node.ranges.push(Range {from: ch0, to: ch2});
                    chars.consume(3);
                },
                [Some(ch0), _, ] => node.chars.push(ch0),
                _ => panic!("RangeNode::parse_node(): should not happen"),
            }
        }
        node.limits = Limits::parse(chars)?;
        Ok(Node::Range(node))
    }

    fn escapes(ch: char) -> char {
        match ch {
            't' => '\t',
            'n' => '\n',
            c => c
        }
    }

    fn matches(&self, string: &str) -> Option<usize> {
        if let Some(ch) = string.chars().next() {
            if self.not != self.chars.contains(ch) || self.ranges.iter().any(|x| x.contains(ch)) {
                Some(char_bytes(string, 1))
            } else { None }
        } else { None }
    }

    fn to_string(&self) -> String {
        let mut string = "[".to_string();
        if self.not { string.push('^')};
        string.push_str(self.chars.as_str());
        for x in self.ranges.iter() {
            string.push_str(x.to_string().as_str());
        }
        string
    }
    fn desc(&self, indent: usize) { println!("{0:1$}{2:?}", "", indent, self); }
}

#[derive(Debug,PartialEq, Clone)]
enum SetUnit {RegularChars(String), SpecialChar(char), Range(char, char), Empty}

impl Debug for AndNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = { if let Some(name) = &self.named { format!("<{}>", name)} else {"".to_string()}};
        write!(f, "AndNode{}({}) {}", name, self.nodes.len(), self.limits.simple_display(), )
    }
}
    
impl AndNode {
    /// Recursively parses an AND node from the front of the Peekable stream
    fn parse_node(chars: &mut Peekable) -> Result<Node, Error> {
        if trace(2) { trace_enter("AND", chars); }
        let named = AndNode::parse_named(chars)?;
        let mut nodes = Vec::<Node>::new();
        loop {
            match chars.peek_2() {
                (None, _) => { return Err(Error::make(2, "Unterminated AND node")); },
                (Some('\\'), Some(')')) => { break; },
                _ => (),
            }
            nodes.push(parse(chars, false)?);
        }

        // pop off terminating chars
        let (_, _) = (chars.next(), chars.next());
        Ok(if nodes.is_empty() { Node::None }
           else { Node::And(AndNode {nodes, limits: Limits::parse(chars)?, named, anchor: false, })})
    }

    /// Parses out the name from a named And
    fn parse_named(chars: &mut Peekable) -> Result<Option<String>, Error> {
        match chars.peek_2() {
            // named match
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
            // silent match: make no record of it
            (Some('?'), _) => {
                chars.consume(1);
                Ok(None)
            },
            // nameless match
            _ => Ok(Some("".to_string())),
        }
    }

    /// recovers an AndNode from the Node::And enum
    fn mut_from_node(node: &mut Node) -> &mut AndNode {
        if let Node::And(and_node) = node { and_node }
        else { panic!("trying to get AndNode from wrong type") }
    }

    fn desc(&self, indent: usize) {
        println!("{0:1$}{2:?}", "", indent, self);
        for i in 0..self.nodes.len() {
            self.nodes[i].desc(indent + TAB_SIZE);
        }
    }
}

impl Debug for OrNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = { if let Some(name) = &self.named { format!("<{}>", name)} else {"".to_string()}};
        write!(f, "OrNode{}({}) {} ", name, self.nodes.len(), self.limits.simple_display(),  )
    }
}

impl OrNode {
    /// Recursively parses an AND node from the front of the Peekable stream
    fn parse_node(chars: &mut Peekable, preceding_node: Node) -> Result<Node, Error> {
        if trace(2) { trace_enter("OR", chars); }
        let mut nodes = vec![preceding_node];
        match parse(chars, true)? {
            Node::Or(mut or_node) => nodes.append(&mut or_node.nodes),
            next_node => nodes.push(next_node),
        };
        // TODO: change return type (extra value no longer needed)
        Ok(Node::Or(OrNode {nodes, limits: Limits::default(), named: None}))
    }
    
    /// recovers an OrNode from the Node::Ar enum
    fn mut_from_node(node: &mut Node) -> &mut OrNode {
        if let Node::Or(or_node) = node { or_node }
        else { panic!("trying to get OrNode from wrong type") }
    }
    
    fn desc(&self, indent: usize) {
        println!("{0:1$}{2:?}", "", indent, self);
        for i in 0..self.nodes.len() {
            self.nodes[i].desc(indent + TAB_SIZE);
        }
    }
}

impl Debug for DefNode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "DefNode({})", self.name, )
    }
}

impl DefNode {
    fn alt_parse_node(chars: &mut Peekable, defs: &Defs) -> Result<Node, Error> { 
        if trace(2) { trace_enter("DEF", chars); }
        let name = Defs::name_from_stream(chars, false);
        if name.is_empty() { return Err(Error::make(100, "Missing required name for RE load")); }
        if let Some(')') = chars.skip_whitespace().next() {}
        else { return Err(Error::make(101, "Bad char in definition name")); }
        if let Some(root) = defs.get(&name) {
            Ok(Node::Def(DefNode{name, node: Box::new(root)}))
        } else { Err(Error::make(102, "Could not find node matching name")) }
    }
    fn desc(&self, indent: usize) {
        println!("{0:1$}{2:?}", "", indent, self);
        self.node.desc(indent + TAB_SIZE);
    }

    fn walk<'a>(&'a self, matched: Matched<'a>) -> walk::Path<'a> { self.node.walk(matched) }
}

//////////////////////////////////////////////////////////////////
//
// parse()
//
// This gets its own section because it is the core of the tree parser. It peeks at the
// next char or so, decides what it is, and calls the proper parse_node to parse the enext node.
//
//////////////////////////////////////////////////////////////////

/// Main entry point for parsing tree
///
/// This prepares the string for processing by making a Peekable object to control the input string. It effectively
/// wraps the whole RE in a \(...\) construct to make a single entry point to the tree.
pub fn parse_tree(input: &str, alt_parser: bool ) -> Result<Node, Error> {
    trace_set_indent(0);
    // wrap the string in "\(...\)" to make it an implicit AND node
    let anchor_front = input.starts_with('^');
    let mut chars = Peekable::new(&input[(if anchor_front {1} else {0})..]);
    let mut outer_and =
        if alt_parser {
            let mut defs = Defs::default();
            chars.push_str(" )");
            AndNode::alt_parse_node(&mut chars, &mut defs)?
        } else {
            chars.push_str(r"\)");
            AndNode::parse_node(&mut chars)?
        };
    if anchor_front {
        let and_node = AndNode::mut_from_node(&mut outer_and);
        and_node.anchor = true;
    }
    if !outer_and.is_none() {
        outer_and.set_named(Some("".to_string()));
        if chars.next().is_some() { return Err(Error::make(5, "Extra characters after parse completed")); }
    }
    Ok(outer_and)
}

/// main controller for the tree parse processing, it looks at the next few characters in the pipeline, decides what they are, and
/// distributes them to the proper XNode constructor function
fn parse(chars: &mut Peekable, after_or: bool) -> Result<Node, Error> {
    let node = match chars.peek_2() {
        (None, _) => Node::None,
        (Some('\\'), Some('(')) => AndNode::parse_node(chars.consume(2))?,
        (Some('\\'), Some(ch1)) => {
            if SpecialNode::ESCAPE_CODES.contains(ch1) { SpecialNode::parse_node(chars)? }
            else { CharsNode::parse_node(chars, after_or)? }
        },
        (Some('.'), _) => SpecialNode::parse_node(chars)?,
        (Some('$'), _) => SpecialNode::parse_node(chars)?,
        (Some('['), _) => RangeNode::parse_node(chars.consume(1))?,
        (_, _) => CharsNode::parse_node(chars, after_or)?,
    };
    if node == Node::None { return Err(Error::make(9, format!("Parse error at \"{}\"", chars.preview(6)).as_str()));}
    if let (Some('\\'), Some('|')) = chars.peek_2() {
        Ok(OrNode::parse_node(chars.consume(2), node)?)
    } else { Ok(node.trace())}
}

/// This is the entrypoint to the phase 2, (tree walk) processing. It is put in this package to make it easier available, since loically it is
/// part of the regexp search functionality.
pub fn walk_tree<'a>(tree: &'a Node, text: &'a str) -> Result<Option<(walk::Path<'a>, usize)>, Error> {
    trace_set_indent(0);
    let mut start_pos = 0;
    // hey, optimization
    // deosn't save that much time but makes the trace debug easier to read
    let root = {if let Node::And(r) = tree { r } else { return Err(Error::make(6, "Root of tree should be Node::And")); }};

    if !root.anchor {
        if let Node::Chars(chars_node) = &root.nodes[0] {
            if chars_node.limits.min > 0 {
                match text.find(chars_node.string.as_str()) {
                    Some(offset) => {
                        if offset > 0 {
                            if trace(1) { println!("\nOptimization: RE starts with \"{}\", skipping {} bytes", chars_node.string, offset); }
                            start_pos = offset;
                        }
                    },
                    None => { return Ok(None); }
                }
            }
        }
    }

    let mut char_start = 0;
    loop {
        if trace(1) {println!("\n==== WALK \"{}\" ====", &text[start_pos..])}
        let matched = Matched { full_string: text, start: start_pos, end: start_pos, char_start };
        let path = tree.walk(matched);
        if path.len() > 1 {
            if trace(1) { println!("--- Search succeeded ---") };
            return Ok(Some((path, char_bytes(text, start_pos))));
        }
        if trace(1) {println!("==== WALK \"{}\": no match ====", &text[start_pos..])};
        if root.anchor { break; }
        if let Some(ch0) = text[start_pos..].chars().next() {
            start_pos += String::from(ch0).len();
            char_start += 1;
        } else {
            break;
        }
    }
    Ok(None)
}

//////////////////////////////////////////////////////////////////
//
// Alternate parser
//
// By breaking the search up into a parse phase and a walk phase it makes implementing
// alternative parsers practical. The parser here is a simplified one allowing for
// construction of more complex searches to be made more easily. The rules are as follows:
//
// - There are kinds of search units: CHAR blocks, RANGE blocks, AND blocks, and OR blocks.
// - Outside of blocks whitespace is ignored. This means carriage returns and indenting
//   can be used to help suggest the organization of a RE
// - Unlike traditional REs, every unit can be saved, with or without a name. In addition,
//   all blocks can have an associated rep coun.
// - Units are indicated as follows:
//   - CHAR unit: "TEXT..." (surrounded by quotation marks.
//     - There are no special characters inside the quotation marks except '\'
//     - Escaped characters include the standard ones for REs (\d for decimal, \a for ascii,
//       etc. In addition, matching anything is "\." (not '.'), and a quote is "\""
//   - RANGE unit: [ABC-M...] or [^ABC-N...]: like traditional REs, this is a list of
//       characters and ranges that either contains or, if beginning with '^', doesn't
//       contain a character
//   - AND unit: and(U!U@U#...\) (starting with "and(" and ending with "\)") like retraditional
//       REs, contains a list of 0 or more units that must all match sequentially
//   - OR unit: or(U!U@U#...\): (starting with "or(" and ending with "\)") Like Retraditional
//       REs, contains a list of 0 or more units where exactly one will match
// - To save a unit in the results it can be either named or unnamed. Names are assigned
//   by following the unit definition with "<NAME>". If NAME is left blank ("<>" it is
//   unnamed but recorded. Anything without a name, aside from the entire match, will not
//   be recorded in the search results
// - Like REs, repetition counts are given by adding a suffix of *, ?, +, {X}, {X,}, or {X,Y}.
//   Likewise, lazy evaluation is signalled by a trailing '?'.
// - The order of the trailing attributes is important. A report including the unit U<name>*
//   will have a report with N named entries matching U, while U*<name> will have a single
// entry for U with the name "name" containing N matches for U.

/// main controller for the tree parse processing, it looks at the next few characters in the pipeline, decides what they are, and
/// distributes them to the proper XNode constructor function
fn alt_parse(chars: &mut Peekable, defs: &mut Defs) -> Result<Node, Error> {
    let mut node = match chars.skip_whitespace().peek_n(4)[..] {
        // define, insert, save, load definitions
        [Some('d'), Some('e'), Some('f'), Some('(')] => defs.parse(chars.consume(4))?,
        [Some('g'), Some('e'), Some('t'), Some('(')] => DefNode::alt_parse_node(chars.consume(4), defs)?,
        [Some('u'), Some('s'), Some('e'), Some('(')] => defs.load(chars.consume(4))?,
        // and, or, various text
        [Some('a'), Some('n'), Some('d'), Some('(')] => AndNode::alt_parse_node(chars.consume(4), defs)?,
        [Some('o'), Some('r'), Some('('), _] => OrNode::alt_parse_node(chars.consume(3), defs)?,
        [Some('"'), _, _, _] => CharsNode::alt_parse_node(chars.consume(1), "\"")?,
        [Some('\''), _, _, _] => CharsNode::alt_parse_node(chars.consume(1), "'")?,
        [Some('t'), Some('x'), Some('t'), Some('(')] => CharsNode::alt_parse_node(chars.consume(4), r")")?,
        [_, _, _, _] => CharsNode::alt_parse_node(chars, "")?,
        _ => return Err(Error::make(101, format!("Unexpected chars in regular expression: \"{}\"", chars.preview(6)).as_str()))
    };
    if !node.is_none() {
        node.set_named(alt_parse_named(chars)?);
        node.set_limits(Limits::parse(chars)?);
        if node.named().is_none() { 
            node.set_named(alt_parse_named(chars)?);
        }
    }
    Ok(node.trace())
}

impl CharsNode {
    const ALT_ESCAPE_CODES: &str = "adntluxo.$";
    fn alt_parse_node(chars: &mut Peekable, terminate: &str) -> Result<Node, Error> {
        if trace(2) { trace_enter("CHARS", chars); }
        /*
        let mut node = CharsNode { blocks: Vec::<CharsContents>::new(), limits: Limits::default(), named: None };
        let mut nodes = Vec::<Node>::new();
        loop {
            let peek3 = chars.peek_n(3);
            if terminate.is_empty() {
                if peek3[0].unwrap_or('x') <= ' ' || peek3[1].is_none() { break; }
            } else if chars.preview(3).starts_with(terminate) {
                chars.consume(terminate.len());
                break;
            }
            match peek3[..] {
                [Some('\\'), Some(ch1), _] => node.push_char(chars.consume(1), CharsNode::ALT_ESCAPE_CODES.contains(ch1)),
                [Some(_), _, _] => node.push_char(chars, false),
                _ => { return Err(Error::make(100, "unterminated string")); },
            }
            if "?*+{".contains(chars.peek().unwrap_or('x')) {
                node = node.handle_rep(chars, &mut nodes)?;
            }
        }
        Ok(if nodes.is_empty() {
            if node.blocks.is_empty() {Node::None}
            else {Node::Chars(node)}
        } else {
            nodes.push(Node::Chars(node));
            Node::And(AndNode {nodes, limits: Limits::default(), named: None, anchor: false})
    })
         */
        Ok(Node::None)
    }
/*
    fn handle_rep(mut self, chars: &mut Peekable, nodes: &mut Vec<Node>) -> Result<CharsNode, Error> {
        let limits = Limits::parse(chars)?;
        let block = self.blocks.pop().unwrap();
        let new_node: Node;
        if let CharsContents::Regular(mut string) = block {
            let last_char = string.pop().unwrap();
            if !string.is_empty() {
                self.blocks.push(CharsContents::Regular(string));
            }
            new_node = Node::Chars( CharsNode{blocks: vec![CharsContents::Regular(format!("{}", last_char))], limits, named: None});
        } else {
            new_node = Node::Chars( CharsNode{ blocks: vec![block], limits, named: None});
        }
        if !self.blocks.is_empty() { nodes.push(Node::Chars(self)); }
        nodes.push(new_node);
        Ok(CharsNode { blocks: Vec::<CharsContents>::new(), limits: Limits::default(), named: None })
}
    */
}

impl AndNode {
    /// Recursively parses an AND node from the front of the Peekable stream
    fn alt_parse_node(chars: &mut Peekable, defs: &mut Defs) -> Result<Node, Error> {
        if trace(2) { trace_enter("AND", chars); }
        let mut nodes = Vec::<Node>::new();
        loop {
            match chars.next() {
                None => { return Err(Error::make(2, "Unterminated AND node")); },
                Some(')') => { break; },
                Some(' ') |
                Some('\n') |
                Some('\t') => (),
                Some(ch) => {
                    chars.put_back(ch);
                    let node = alt_parse(chars, defs)?;
                    if !node.is_none() {
                        nodes.push(node);
                    }
                }
            }
        }
        if nodes.is_empty() { Ok(Node::None) }
        else { Ok(Node::And(AndNode {nodes, limits: Limits::default(), named: None, anchor: false, })) }
    }

}

impl OrNode {
    fn alt_parse_node(chars: &mut Peekable, defs: &mut Defs) -> Result<Node, Error> {
        if trace(2) { trace_enter("OR", chars); }
        let mut nodes = Vec::<Node>::new();
        loop {
            match chars.next() {
                None => { return Err(Error::make(2, "Unterminated OR node")); },
                Some(')') => { break; },
                Some(' ') |
                Some('\n') |
                Some('\t') => (),
                Some(ch) => {
                    chars.put_back(ch);
                    let node = alt_parse(chars, defs)?;
                    if !node.is_none() {
                        nodes.push(node);
                    }
                }
            }
        }
        if nodes.is_empty() { Ok(Node::None) }
        else { Ok(Node::Or(OrNode {nodes, limits: Limits::default(), named: None, })) }
    }        
}

/// Parses out the name from a named And
fn alt_parse_named(chars: &mut Peekable) -> Result<Option<String>, Error> {
    if chars.peek() != Some('<') { return Ok(None); }
    chars.consume(1);
    let mut chs = Vec::<char>::new();
    loop {
        match (chars.next(), chars.peek()) {
            (Some('>'), _) => { break; },
            (Some('\\'), Some(')')) => { return Err(Error::make(7, "Unterminated name in AND definition")); },
            (Some(ch), _) => chs.push(ch),
            _ => { return Err(Error::make(8, "error getting name in AND definition")); },
        }
    }
    Ok(Some(chs.into_iter().collect()))
}

#[derive(Default)]
struct Defs {
    defs: HashMap<String, Node>,
    load: Vec<String>,
    loaded: Vec<String>,
}

impl Defs {
    fn parse(&mut self, chars: &mut Peekable) -> Result<Node, Error>{
        let name = Defs::name_from_stream(chars, false);
        if name.is_empty() { return Err(Error::make(100, "Missing required name for RE definition")); }

        if trace(2) { trace_indent(); println!("reading definition of {}", name); trace_change_indent(1); }
        let mut nodes = Vec::<Node>::new();
        loop {
            let node = alt_parse(chars, self)?;
            if !node.is_none() {
                nodes.push(node);
            }
            chars.skip_whitespace();
            if let Some(')') = chars.peek() {
                chars.consume(1);
                break;
            }
        }
        if nodes.is_empty() { return Err(Error::make(202, "No valid definition found")) }
        let root = if nodes.len() == 1 {
            nodes.into_iter().next().unwrap()
        } else {
            Node::And(AndNode{limits: Limits::default(), named: None, anchor: false, nodes})
        };
        self.defs.insert(name, root);
        if trace(2) { trace_change_indent(-1); trace_indent(); println!("finished definition"); }
        Ok(Node::None)
    }

    fn get(&self, name: &str) -> Option<Node> {
        self.defs.get(name).cloned()
        //        if let Some(node) = self.defs.get(name) {Some(node.clone()) }
//        else { None}
            /*
        let name = Defs::name_from_stream(chars, false);
        if name.is_empty() { return Err(Error::make(100, "Missing required name for RE load")); }
        if let Some(')') = chars.skip_whitespace().next() {}
        else { return Err(Error::make(101, "Bad char in definition name")); }
//        if let Some(root) = self.defs.get(&name) { Ok(root.clone()) }
        if let Some(root) = self.defs.get(&name) {
            Ok(Node::Def(DefNode{name, node: Box::new(root.clone())}))
        }
        else { Err(Error::make(102, "Could not find node matching name")) }
            */
    }

    fn load(&mut self, chars: &mut Peekable) -> Result<Node, Error> {
        let path = Defs::path_from_stream(chars);
        if let Some(')') = chars.skip_whitespace().next() {}
        else {return Err(Error::make(202, "Malformed \"use\" statement"));}
        if trace(1) { trace_indent(); println!("loading definitions from file '{:#?}'", path); trace_change_indent(1); }
        
        match std::fs::read_to_string(&path) {
            Err(err) => { return Err(Error::make(202, format!("Error reading def file {}: {}", path, err.to_string()).as_str())) },
            Ok(string) => {
                let mut def_chars = Peekable::new(&string);
                let _ = alt_parse(&mut def_chars, self)?;
            }
        }
        if trace(1) {
            if trace(2) { trace_indent(); println!("finished load of '{:#?}'", path);}
            trace_change_indent(-1);
        }
        Ok(Node::None)
        
    }
        
    fn name_from_stream(chars: &mut Peekable, file: bool) -> String {
        let mut name_v = Vec::<char>::new();
        chars.skip_whitespace();
        loop {
            if let Some(ch) = chars.next() {
                if ('a'..='z').contains(&ch)||
                    ('A'..='Z').contains(&ch)||
                    ('0'..='9').contains(&ch)||
                    "_$#.".contains(ch) ||
                    (file && "/~~".contains(ch))
                {
                    name_v.push(ch);
                } else {
                    chars.put_back(ch);
                    break;
                }
            }
        }
        chars.skip_whitespace();
        name_v.iter().collect::<String>()
    }
    
    fn path_from_stream(chars: &mut Peekable) -> String {
        let name = Defs::name_from_stream(chars, true);
        let name = if name.is_empty() {"~/.regexp".to_string()} else { name };
        if name[0..2].eq("~/") {
            let home = home::home_dir();
            let mut home_str = home.unwrap().display().to_string();
            home_str.push_str(&name[1..]);
            home_str
        } else {
            name
        }
    }
}

//////////////////////////////////////////////////////////////////
//
// Helper functions
//
//////////////////////////////////////////////////////////////////

/// gets the number of bytes in a sring of unicode characters
fn char_bytes(string: &str, char_count: usize) -> usize {
    let s: String = string.chars().take(char_count).collect();
    s.len()
}

//////////////////////////////////////////////////////////////////
//
// LIMITS
//
/// Used to handle the number of reps allowed for a Node. Besides holding the min, max, and lazy data,
/// it also handles other related questions, like whether a node falls in the allowed range, or how
/// far the initial walk should go
//
//////////////////////////////////////////////////////////////////
#[derive(Debug,Clone,Copy,PartialEq)] 
/// Holds and handles the limit information for a Node: the min and max repetitions allowed, and whether
/// it is lazy or not.
/// **IMPORTANT**: MIN and MAX are the actual sizes allowed (that is, ? is min 0, max 1). But the check()
/// method takes as input the number of Steps in the Path. Since there is an entry for 0 steps the number
/// passed to check() is actually one higher than the actual repetition count (this is because the arg
/// passed in is USIZE, and needs to handle a < 0 condition when 0 reps does not match). However this is
/// handled it causes confusion somewhere, this way handling is limited to the check() method.
pub struct Limits {
    min: usize,
    max: usize,
    lazy: bool,
}

impl Default for Limits { fn default() -> Limits { Limits{min: 1, max: 1, lazy: false} } }

impl std::fmt::Display for Limits {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut reps = match (self.min, self.max) {
            (1, 1) => "".to_string(),
            (0, 1) => "?".to_string(),
            (0, EFFECTIVELY_INFINITE) => "*".to_string(),
            (1, EFFECTIVELY_INFINITE) => "+".to_string(),
            (min, EFFECTIVELY_INFINITE) => format!("{{{},}}", min),
            (min, max) => { if min == max { format!("{{{}}}", min) } else {format!("{{{},{}}}", min, max)} },
        };
        if self.lazy { reps.push('?'); }
        f.write_str(reps.as_str())
    }
}

impl Limits {
    /// Display every Limit in a *{min, max}* format for debugging
    fn simple_display(&self) -> String { format!("{{{},{}}}{}", self.min, self.max, if self.lazy {"?"} else {""})}

    /// returns a Limit struct parsed out from point. If none is there returns the default
    /// Like parse_if() but always returns astruct, using the default if there is none in the string
    fn parse(chars: &mut Peekable) -> Result<Limits, Error> {
        let next = chars.next();
        if next.is_none() { return Ok(Limits::default()); }
        let next = next.unwrap();
        let (min, max): (usize, usize) = match next {
            '*' => (0, EFFECTIVELY_INFINITE),
            '+' => (1, EFFECTIVELY_INFINITE),
            '?' => (0, 1),
            '{' => Limits::parse_ints(chars)?,
            _ => { chars.put_back(next); return Ok(Limits::default())}
        };
        let lazy = chars.peek().unwrap_or('x') == '?';
        if lazy { let _ = chars.next(); }
        Ok(Limits{min, max, lazy})
    }

    /// helper function to parse an int at the current position of the RE being parsed
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

    /// Checks if the size falls in the range.
    /// Returns: <0 if NUM is < min; 0 if NUM is in the range min <= NUM <= ,ax (but SEE WARNING BELOW: NUM needs
    /// to be adjusted to account for the 0-match possibility.
    ///
    /// **Beware**: the input is usize and is in general the length of steps vector.
    /// This has a 0-match in its first position, so the value entered is actually one higher than the allowed value.
    /// I considered subtracting 1 from the number to make it match, but because the arg is usize and is sometimes 0
    /// it is better to leave it like this.
    pub fn check(&self, num: usize) -> isize {
        if num <= self.min { -1 }
        else if num <= self.max + 1 { 0 }
        else { 1 }
    }

    /// gives the length of the initial walk: MAX for greedy, MIN for lazy
    pub fn initial_walk_limit(&self) -> usize { if self.lazy {self.min} else { self.max}}
}
//
// These functions parse the reps option from the re source
//

/// reads an int from input, consuming characters if one is there, otherwise not changing anything
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
// Report
//
/// Used to deliver the search results to the caller. Results form a tree, AndNode and OrNode are branches, the other
/// Nodes are leaves. **Report** is built up from the successful **Path** that walked the entire tree.
//
//////////////////////////////////////////////////////////////////

#[derive(Debug,Clone)]
pub struct Report {
    /// The string found matching the RE pattern
    pub found: String,
    /// The position in chars of the string. This cannot be used for slices, except for ASCII chars. To get a slice use **pos**
    pub pos: (usize, usize),
    /// The position in bytes of the string: that is, found[pos.0..pos.1] is a valid unicode substring containing the match
    pub bytes: (usize, usize),
    /// The name of the field: if None then the field should not be included in the Report tree, if Some("") it is included but
    /// unnamed, otherwise it is recorded with the given name
    pub name: Option<String>,
    /// Array of child Report structs, only non-empty for And and Or nodes. OrNodes will have only a single child node, AndNodes can have many.
    pub subreports: Vec<Report>,
}

impl Report {
    /// Constructor: creates a new report from a successful Path
    pub fn new(root: &crate::regexp::walk::Path, char_start: usize) -> Report {
        let (reports, _char_end)  = root.gather_reports(char_start);
        reports[0].clone()
    }
    
    /// Pretty-prints a report with indentation to help make it easier to read
    pub fn display(&self, indent: usize) {
        let name_str = { if let Some(name) = &self.name { format!("<{}>", name) } else { "".to_string() }};
        print!("{0:1$}", "", indent);
        println!("\"{}\" char position [{}, {}] byte position [{}, {}] {}",
                 self.found, self.pos.0, self.pos.1, self.bytes.0, self.bytes.1, name_str);
        self.subreports.iter().for_each(move |r| r.display(indent + TAB_SIZE));
    }

    /// Gets **Report** nodes representing matches for named Nodes. The return is a *Vec* because named matches can occur multiple
    /// times - for example, _\?\<name\>abc\)*_
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

    /// Gets a hash of  **Report** nodes grouped by name. This just sets things up and calls **get_named_internal()** to do the work
    pub fn get_named(& self) -> HashMap<&str, Vec<&Report>> {
        let hash = HashMap::new();
        self.get_named_internal(hash)
    }

    /// internal function that does the work for **get_named()**
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

//////////////////////////////////////////////////////////////////
//
/// Peekable
///
/// This is an iterator with added features to make linear parsing of the regexp string easier:
///     1) peeking: the next char can be peeked (read without consuming) or returned after being consumed
///     2) extra characters can be added to the stream at the end of the buffer (without copying the entire string)
///
/// It also has progress(), a sanity check to catch suspicious behavior, like infinite loops or overuse of peeking
//
//////////////////////////////////////////////////////////////////

#[derive(Debug)]
struct Peekable<'a> {
    /// The char iterator sourcing the chars
    chars: Chars<'a>,
    /// a vector holding characters taken off of *chars* but not consumed. Requests to **next()** grab input from here before looking in **chars**.
    peeked: Vec<char>,
    /// A vector holding chars appended to the end of the input string. This is only accessed after the **chars** iterator has been exhausted.
    trailer: Vec<char>,
    /// To minimize the chance of infinite loops this is inc'ed whenever a char is read. This way if no progress is made in processing the RE
    /// string a warning can be sent. I worry there could be some bad syntax that causes an infinite loop, this should cach such a happening.
    progress_check: isize,
}

impl<'a> Peekable<'a> {
    /// sanity check: if peeked stack exceeds this size it is probably a problem
    const PEEKED_SANITY_SIZE: usize = 20;
    /// create a new **Peekable** to source a string
    fn new(string: &str) -> Peekable { Peekable { chars: string.chars(), peeked: Vec::<char>::new(), trailer: Vec::<char>::new(), progress_check: 1} }

    /// gets the next char from the **Peekable** stream - first checks **peeked**, then **chars**, finally **trailer**
    pub fn next(&mut self) -> Option<char> {
        if !self.peeked.is_empty() { Some(self.peeked.remove(0)) }
        else { self.next_i() }
    }

    /// peek() looks at the next character in the pipeline. If called multiple times it returns the same value
    pub fn peek(&mut self) -> Option<char> {
        if self.peeked.is_empty() {
            let ch = self.next_i()?;
            self.peeked.push(ch);
        }
        Some(self.peeked[0])
    }

    /// peek at the next n chars
    pub fn peek_n(&mut self, n: usize) -> Vec<Option<char>> {
        let mut ret: Vec<Option<char>> = Vec::new();
        for ch in self.peeked.iter() {
            if ret.len() == n { return ret; }
            ret.push(Some(*ch));
        }
        while ret.len() < n { ret.push(self.peek_next()); }
        ret
    }

    /// convenient because 2 chars is all the lookahead I usually need
    pub fn peek_2(&mut self) -> (Option<char>, Option<char>) {
        let x = self.peek_n(2);
        (x[0], x[1])
    }


    /// This simply adds the char back in the queue. It is assumed the caller returns the chars in the reverse order they are popped off
    pub fn put_back(&mut self, ch: char) {
        self.progress_check -= 1;
        self.peeked.insert(0, ch);
    }

    pub fn put_back_str(&mut self, string: &str) {
        self.progress_check -= 1;
        for ch in string.chars().rev() {
            self.peeked.insert(0, ch);
        }
    }

    /// pushed a char onto the back of the **Peekable** stream
    pub fn push(&mut self, ch: char) { self.trailer.push(ch); }

    /// pushed a char onto the back of the **Peekable** stream
    pub fn push_str(&mut self, string: &str) { string.chars().for_each(move |ch| self.trailer.push(ch)); }

    /// (as the name says) skips over whitespace at the front of the stream
    pub fn skip_whitespace(&mut self) -> &mut Self {
        loop {
            if let Some(ch) = self.next() {
                if !" \n\t".contains(ch) {
                    self.put_back(ch);
                    break;
                }
            }
        }
        self
    }

    /// disploses of the front **num** characters from the stream
    pub fn consume(&mut self, num: usize) -> &mut Self { for _i in 0..num { let _ = self.next(); } self}
    
    /// simple to do, and maybe useful for early stages: make sure the parse loop can't get through without burning at least one character
    fn progress(&mut self) {
        if self.progress_check <= 0 {panic!("Looks like no progress is being made in parsing string"); }
        if self.peeked.len() > Peekable::PEEKED_SANITY_SIZE { panic!("PEEKED stack has grown to size {}", self.peeked.len()); }
        self.progress_check = 0;
    }
    
    /// **next_internal()**, fetches the next char from the iterator, or the trailer if the iterator is exhausted
    fn next_i(&mut self) -> Option<char> {
        let mut ret = self.chars.next();
        if ret.is_none() {
            ret = if self.trailer.is_empty() { None } else { Some(self.trailer.remove(0)) };
        }
        self.progress_check += 1;
        ret
    }
            
    /// peek_next() gets the next unread character, adds it to the peeked list, and returns it
    fn peek_next(&mut self) -> Option<char> {
        let ch = self.next_i();
        if let Some(c) = ch { self.peeked.push(c); }
        ch
    }

    /// get a string of the first **len** chars from the stream
    fn preview(&mut self, len: usize) -> String {
        self.peek_n(len).iter().filter_map(|x| *x).collect::<String>()
    }
}

/// simple struct used to provide control on how errors are displayed
#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub code: usize,
}

impl Error {
    /// constructor
    fn make(code: usize, msg: &str,) -> Error { Error{code, msg: msg.to_string()}}
}

impl core::fmt::Display for Error {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Error:{}: {}", self.code, self.msg)
    }
}
