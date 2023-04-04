//! ## Regular expression search: Tree walker
//! This module contains the code for walking the tree to find a RE match. Each match of a **Node** (representing a unit in the
//! RE tree) is represented by a **Step** object. The **Step**s are grouped in vectors to form **Path**s, each of which represents
//! a walk through the tree. When a **Path** reaches the end of the tree successfully it means the search has succeeded and that
//! **Path* is returned, representing a matched string, so it can generate a **Report** giving its route.
use super::*;
use crate::{trace, trace_indent, trace_change_indent};

/// simplifies code a little by removing the awkward accessing of the last element in a vector
fn ref_last<T>(v: &Vec<T>) -> &T { &v[v.len() - 1] }

//////////////////////////////////////////////////////////////////
//
// Step structs
//
// A path through the tree is a vector of steps, where each step is a
// single match for its node.  The steps are used to walk through the
// trtee and to build the Report structure for the caller.
//
//////////////////////////////////////////////////////////////////
/// Represents a single step for a CharsNode
pub struct CharsStep<'a> {
    /// The node from phase 1
    node: &'a CharsNode,
    /// the String where this match starts
    string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    match_len: usize,
}

pub struct SpecialStep<'a> {
    /// The node from phase 1
    node: &'a SpecialCharNode,
    /// the String where this match starts
    string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    match_len: usize,
}

pub struct SetStep<'a> {
    /// The node from phase 1
    node: &'a SetNode,
    /// the String where this match starts
    string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    match_len: usize,
}

pub struct AndStep<'a> {
    /// The node from phase 1
    node: &'a AndNode,
    /// the String where this match starts
    string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    match_len: usize,
    /// A vector of Paths saving the current state of this And node. Each entry is a **Path** based on the **nodes** member of the **AndNode** structure.
    /// When the Paths vector is filled this step for the And node has succeeded.
    child_paths: Vec<Path<'a>>,
}

pub struct OrStep<'a> {
    /// The node from phase 1
    node: &'a OrNode,
    /// the String where this match starts
    string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    match_len: usize,
    /// The OR node needs only a single branch to succeed. This holds the successful path
    child_path: Box<Path<'a>>,
    /// This points to the entry in **node.nodes** which is currently in the **child_path** element
    which: usize,
}

//////////////////////////////////////////////////////////////////
//
// Path struct
//
/// Conceptually a Path takes as many steps as it can along the target
/// string. A Path is a series of Steps along a particular branch, from
/// 0 up to the maximum number allowed, or the maximum number matched,
/// whichever is smaller. If continuing the search from the end of the
/// Path fails it backtracks a Step and tries again.
//
//////////////////////////////////////////////////////////////////
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>), Special(Vec<SpecialStep<'a>>), Set(Vec<SetStep<'a>>), And(Vec<AndStep<'a>>), Or(OrStep<'a>), None }

impl<'a> Path<'a> {
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    
    /// length of the **Path** (the number of **Step**s it has)
    pub fn len(&self) -> usize {
        match self {
            Path::Chars(steps) => steps.len(),
            Path::Special(steps) => steps.len(),
            Path::Set(steps) => steps.len(),
            Path::And(steps) => steps.len(),
            Path::Or(step) => step.node.nodes.len() - step.which,
            Path::None => 0,
        }
    }

    /// the length of the match, in bytes. This means it can be used to extract the unicode string from the string element
    fn match_len(&self) -> usize {
        match self {
            Path::Chars(steps) =>    steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::Special(steps) =>  steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::Set(steps) =>      steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::And(steps) =>      steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::Or(step) =>        step.match_len ,
            Path::None =>            0,
        }
    }
    
    /// gets the subset of the target string matched by this **Path**
    pub fn matched_string(&self) -> &'a str {
        let match_len = self.match_len();
        match self {
            Path::Chars(steps) =>    &steps[0].string[0..match_len],
            Path::Special(steps) =>  &steps[0].string[0..match_len],
            Path::Set(steps) =>      &steps[0].string[0..match_len],
            Path::And(steps) =>      &steps[0].string[0..match_len],
            Path::Or(step) =>        &step.string[0..match_len],
            Path::None => panic!("NONE unexpected"),
        }
    }

    // returns the end of the string matched by this path
    fn string_end(&self) -> &'a str {
        let len = self.match_len();
        match self {
            Path::Chars(steps) =>    &steps[0].string[len..],
            Path::Special(steps) =>  &steps[0].string[len..],
            Path::Set(steps) => &steps[0].string[len..],
            Path::And(steps) =>      &steps[0].string[len..],
            Path::Or(step) =>        &step.string[len..],
            Path::None => panic!("NONE unexpected"),
        }
    }

    /// backs off a step on the path: when a path fails the process backtracks a step and tries again. This
    /// actually handles both the lazy and the greedy cases: for greedy the initial path walks to the upper limit
    /// or to failure, whichever is smaller, and backs off by removing a step from the path. For the lazy case
    /// the initial path is the minimum length, and it case of failure it adds another step to the end, up to the
    /// maximum limit
    fn pop(&mut self) -> bool {
        if let Path::And(steps) = self {
            let last = steps.len() - 1;
            let children = &mut steps[last].child_paths;
            let len = children.len();
            if !children.is_empty() && children[len - 1].pop() { return true; }
        };
        let limits = self.limits();
        if limits.lazy { return self.lazy_pop(); }
        match self {
            Path::Chars(steps) =>    { let _ = steps.pop(); },
            Path::Special(steps) =>  { let _ = steps.pop();},
            Path::Set(steps) =>      { let _ = steps.pop();},
            Path::And(steps) =>      { let _ = steps.pop();},
            Path::None =>            panic!("NONE unexpected"),
            Path::Or(step) =>        {
                let child_path = &mut step.child_path;
                if !child_path.pop() { step.which += 1; }
                step.match_len = if child_path.len() == 0 {0} else {child_path.match_len()};
            },
        };
        let ret = limits.check(self.len()) == 0;
        if trace(3) { println!("{}backoff: {:?}, success: {}", trace_indent(), self, ret)}
        ret
    }

    /// implementation of pop() for lazy evaluation
    fn lazy_pop(&mut self) -> bool {
        self.limits().check(self.len() + 1) == 0
            && match self {
                Path::Chars(steps) =>    { if let Some(step) = steps[steps.len() - 1].step() {steps.push(step); true} else { false}},
                Path::Special(steps) =>  { if let Some(step) = steps[steps.len() - 1].step() {steps.push(step); true} else { false}},
                Path::Set(steps) =>      { if let Some(step) = steps[steps.len() - 1].step() {steps.push(step); true} else { false}},
                Path::And(steps) =>      { if let Some(step) = steps[steps.len() - 1].step() {steps.push(step); true} else { false}},
                Path::None =>            panic!("NONE unexpected"),
                Path::Or(_step) =>       panic!("OR unexpected"),
            }
    }

    /// returns ths **Limit** object for the Path
    fn limits(&self) -> Limits {
        match self {
            Path::Chars(steps) =>    steps[0].node.limits(),
            Path::Special(steps) =>  steps[0].node.limits(),
            Path::Set(steps) =>      steps[0].node.limits(),
            Path::And(steps) =>      steps[0].node.limits(),
            Path::Or(step) =>        step.node.limits(),
            Path::None =>            panic!("Accessign limits() of None node"),
        }
    }

    /// recursively creates **Report** objects for a path. For the branches (And and Or) this means recording itself
    /// and then collecting from the children recursively. leaves just need to record themselves
    pub fn gather_reports(&'a self, char_start: usize, byte_start: usize) -> (Vec<Report>, usize) {
        let mut char_pos = char_start;
        let mut byte_pos = byte_start;
        let mut reports = Vec::<Report>::new();
        match self {
            Path::And(steps) => {
                if steps.len() == 1 {
                    let (mut subreport, _pos,) = steps[0].make_report(char_pos, byte_pos);
                    if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                    else { reports.push(subreport); }
                } else {
                    for step in steps.iter().skip(1) {
                        let (mut subreport, pos) = step.make_report(char_pos, byte_pos);
                        char_pos = pos;
                        byte_pos += step.match_len;
                        if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                        else { reports.push(subreport); }
                    }
                }
            },
            Path::Or(step) => {
                let (subreport, pos) = step.make_report(char_start, byte_start);
                char_pos = pos;
                if subreport.name.is_none() { reports = subreport.subreports; }
                else { reports.push(subreport); }
            },
            _ => {
                char_pos += self.matched_string().chars().count();
            },
        }
        (reports, char_pos)
    }

    /// called when a Path has completed to print its trace, if debugging is enabled
    fn trace(self, level: u32, prefix: &'a str) -> Path {
        if trace(level) {
            trace_change_indent(-1);
            println!("{}{} {:?}", trace_indent(), prefix, &self);
        }
        self
    }
}

//////////////////////////////////////////////////////////////////
//
// Debug implementations: used for tracing
//
//////////////////////////////////////////////////////////////////
impl<'a> Debug for CharsStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CHARS \"{}\"{}, string {}", self.node.string, self.node.limits().simple_display(), abbrev(self.string) )
    }
}
impl<'a> Debug for SpecialStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SPECIAL \"{}\"{}, string {}", self.node.special, self.node.limits().simple_display(), abbrev(self.string))
    }
}
impl<'a> Debug for SetStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SET \"{}\"{}, string {}", self.node.targets_string(), self.node.limits().simple_display(), abbrev(self.string))
    }
}
impl<'a> Debug for AndStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut child_counts: String = "".to_string();
        for p in self.child_paths.iter() { child_counts.push_str(&format!("{}, ", p.len())); }
        for _i in self.child_paths.len()..self.node.nodes.len() { child_counts.push_str("-, "); }
        let name = { if let Some(name) = &self.node.named { format!("<{}>", name) } else { "".to_string()} };
        write!(f, "AND{}({}){} state [{}], string {}", name, self.node.nodes.len(), self.node.limits().simple_display(), child_counts, abbrev(self.string))
    }
}
impl<'a> Debug for OrStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "OR({}){}, branch {}, branch reps {}, string {}", self.node.nodes.len(), self.node.limits().simple_display(), self.which, self.child_path.len(), abbrev(self.string))
    }
}
impl<'a> Debug for Path<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Path::Chars(steps) =>    {
                if steps.is_empty() { write!(f, "CHARS, reps 0, match \"\"") }
                else {write!(f, "{:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())}
            },
            Path::Special(steps) =>  {
                if steps.is_empty() { write!(f, "SPECIAL, reps 0, match \"\"") }
                else { write!(f, "{:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())}
            },
            Path::Set(steps) => {
                if steps.is_empty() { write!(f, "SET, reps 0, match \"\"") }
                else {write!(f, "{:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())}
            },
            Path::And(steps) => {
                if steps.is_empty() { write!(f, "AND, reps 0, match \"\"") }
                else {
                    write!(f, "{:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())
                }
            },
            Path::Or(step) => write!(f, "{:?}, child reps {}, match \"{}\"", step, step.child_path.len(), self.matched_string()),
            Path::None => write!(f, "NONE should not be included in any path"),
        }
    }
}

//////////////////////////////////////////////////////////////////
//
// Step struct method definitions
//
//////////////////////////////////////////////////////////////////

// Any way to make walk() generic?
impl<'a> CharsStep<'a> {
    /// start a Path using a string of chars, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a CharsNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<CharsStep>::new();
        steps.push(CharsStep {node, string, match_len: 0});
        if trace(2) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(1);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(3) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => break,
            }
        }
        Path::Chars(steps).trace(1, "end walk")
    }
    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    /// try to take a single step over a string of regular characters
    fn step(&self) -> Option<CharsStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) { Some(CharsStep {node: self.node, string, match_len: self.node.string.len()}) }
        else { None }
    }
}
        
impl<'a> SpecialStep<'a> {
    /// start a Path using a special char, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a SpecialCharNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<SpecialStep>::new();
        steps.push(SpecialStep {node, string, match_len: 0});
        if trace(1) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(1);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(3) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::Special(steps).trace(1, "end walk")
    }
    /// try to take a single step over a special character
    fn step(&self) -> Option<SpecialStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) { Some( SpecialStep {node: self.node, string, match_len: char_bytes(string, 1)}) }
        else { None }
    }
}

impl<'a> SetStep<'a> {
    /// start a Path using a set to match, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a SetNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<SetStep>::new();
        steps.push(SetStep {node, string, match_len: 0});
        if trace(2) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(1);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(3) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::Set(steps).trace(2, "end walk")
    }
    /// try to take a single step over a set of characters
    fn step(&self) -> Option<SetStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) { Some(SetStep {node: self.node, string, match_len: char_bytes(string, 1)}) } else
        { None }
    }
}

impl<'a> AndStep<'a> {
    /// start a Path using an And node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a AndNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<AndStep>::new();
        steps.push(AndStep {node, string, match_len: 0, child_paths: Vec::<Path<'a>>::new()});
        if trace(2) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(1);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(3) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::And(steps).trace(2, "end walk")
    }

    /// try to take a single step matching an And node
    fn step(&self) -> Option<AndStep<'a>> {
        let string0 = &self.string[self.match_len..];
        let mut step = AndStep {node: self.node,
                                string: string0,
                                match_len: 0,
                                child_paths: Vec::<Path<'a>>::new(),
        };
        loop {
            let child_len = step.child_paths.len();
            if child_len == step.node.nodes.len() { break; }
            let string = if child_len == 0 { step.string }
            else {ref_last(&step.child_paths).string_end()};
            let child_path = step.node.nodes[child_len].walk(string);
            if child_path.limits().check(child_path.len()) == 0 {
                step.child_paths.push(child_path);
                if trace(3) {
                    trace_change_indent(-1);
                    println!("{}new step: {:?}", trace_indent(), step);
                    trace_change_indent(1);
                }
            } else if !step.back_off() {
                return None;
            }
        }
        step.match_len = step.string.len() - ref_last(&step.child_paths).string_end().len();
        Some(step)
    }

    /// back off a step after a failed match: after taking a step back see if the path length still falls within
    /// the desired limits, if it does continue along that new path to see if it succeeds, if not remove that child
    /// and then back off on the preceding child. When all children have been exhausted the step has failed.
    fn back_off(&mut self) -> bool {
        if self.child_paths.is_empty() { return false; }
        loop {
            let last_pathnum = self.child_paths.len() - 1;
            if self.child_paths[last_pathnum].pop() { break; }
            self.child_paths.pop();
            if self.child_paths.is_empty() { break; }
        }
        !self.child_paths.is_empty()
    }

    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&self, char_start: usize, byte_start: usize) -> (Report, usize) {
        let mut reports = Vec::<Report>::new();
        let mut char_end = char_start;
        let mut byte_end = byte_start;
        for p in &self.child_paths {
            let (mut subreports, loc) = p.gather_reports(char_end, byte_end);
            char_end = loc;
            byte_end += p.match_len();
            reports.append(&mut subreports);
        }
        (Report {found: self.string[0..self.match_len].to_string(),
                 name: self.node.named.clone(),
                 pos: (char_start, char_end),
                 bytes: (byte_start, byte_start + self.match_len),
                 subreports: reports},
         char_end)
    }
}
                    
/// OR does not have a *step()* function because it cannot have a repeat count (to repeat an OR it must be enclosed in an AND)
impl<'a> OrStep<'a> {
    /// start a Path using an Or node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a OrNode, string: &'a str) -> Path<'a> {
        if trace(2) {
            let fake = OrStep {node, string, which: 0, child_path: Box::new(Path::None), match_len: 0};
            println!("{}Starting walk for {:?}", trace_indent(), fake);
            trace_change_indent(1);
        }
        for which in 0..node.nodes.len() {
            let child_path = node.nodes[which].walk(string);
            if child_path.limits().check(child_path.len()) == 0 {
                let match_len = child_path.match_len();
                return Path::Or(OrStep {node, string, which, child_path: Box::new(child_path), match_len}).trace(2, "end walk0");
            }
        }
        Path::Or(OrStep {node, string, which: node.nodes.len(), child_path: Box::new(Path::None), match_len: 0}).trace(2, "end walk1")
    }

    /// Compiles a **Report** object from this path and its child after a successful search
    fn make_report(&self, char_start: usize, byte_start: usize) -> (Report, usize) {
        let (subreports, char_end) = self.child_path.gather_reports(char_start, byte_start);
        (Report {found: self.string.to_string(), name: None, pos: (char_start, char_end), bytes: (byte_start, byte_start + self.match_len), subreports}, char_end)
    }
}

/// Set the length a String can be before it is abbreviated
pub fn set_abbrev_size(size: u32) { unsafe {ABBREV_LEN = size as usize; }}
static mut ABBREV_LEN: usize = 5;
/// **abbrev()** is used in the pretty-print of steps: The display prints out the string at the start of the step. If it is too
/// long it distracts from the other output. **abbrev** limits the length of the displayed string by replacing its end with "..."
fn abbrev(string: &str) -> String {
    let s:String = string.chars().take(unsafe {ABBREV_LEN}).collect();
    let dots = if s.len() == string.len() {""} else {"..."};
    format!("\"{}\"{}", s, dots)
}


