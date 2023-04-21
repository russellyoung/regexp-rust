//! ## Regular expression search: Tree walker
//! This module contains the code for walking the tree to find a RE match. Each match of a **Node** (representing a unit in the
//! RE tree) is represented by a **Step** object. The **Step**s are grouped in vectors to form **Path**s, each of which represents
//! a walk through the tree. When a **Path** reaches the end of the tree successfully it means the search has succeeded and that
//! **Path* is returned, representing a matched string, so it can generate a **Report** giving its route.
use super::*;
use crate::{trace, trace_indent, trace_change_indent};

/// simplifies code a little by removing the awkward accessing of the last element in a vector

//////////////////////////////////////////////////////////////////
//
// Step structs
//
// A path through the tree is a vector of steps, where each step is a
// single match for its node.  The steps are used to walk through the
// trtee and to build the Report structure for the caller.
//
//////////////////////////////////////////////////////////////////
#[derive(Debug,Copy)]
pub struct Matched<'a> {
    /// the String where this match starts
    pub full_string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    pub start: usize,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    pub end: usize,
    /// the start of the string in characters
    pub char_start: usize,
}

/// Represents a single step for a CharsNode (a string of regular characters)
pub struct CharsStep<'a> {
    /// The node from phase 1
    node: &'a CharsNode,
    matched: Matched<'a>,
}

/// Represents a single step for a SetNode (characters belonging to a defined set, like [a-z .,]
pub struct SetStep<'a> {
    /// The node from phase 1
    node: &'a SetNode,
    matched: Matched<'a>,
}

/// Represents a single step for an AndNode (a collection of 0 or more nodes that all must match)
pub struct AndStep<'a> {
    /// The node from phase 1
    node: &'a AndNode,
    matched: Matched<'a>,
    /// A vector of Paths saving the current state of this And node. Each entry is a **Path** based on the **nodes** member of the **AndNode** structure.
    /// When the Paths vector is filled this step for the And node has succeeded.
    child_paths: Vec<Path<'a>>,
}

/// Represents a single step for an OrNode (a collection of 0 or more nodes that one must match)
pub struct OrStep<'a> {
    /// The node from phase 1
    node: &'a OrNode,
    matched: Matched<'a>,
    /// The OR node needs only a single branch to succeed. This holds the successful path
    child_path: Box<Path<'a>>,
    /// This points to the entry in **node.nodes** which is currently in the **child_path** element
    which: usize,
}

trait Walker: Debug {}

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
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>), Set(Vec<SetStep<'a>>), And(Vec<AndStep<'a>>), Or(Vec<OrStep<'a>>), None }

impl<'a> Path<'a> {
    // basic methods
    pub fn is_none(&self) -> bool {
        match self {
            Path::None => true,
            _ => false,
        }
    }
    
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    
    /// length of the **Path** (the number of **Step**s it has)
    pub fn len(&self) -> usize {
        match self {
            Path::Chars(steps) => steps.len(),
            Path::Set(steps)   => steps.len(),
            Path::And(steps)   => steps.len(),
            Path::Or(steps)    => steps.len(),
            Path::None => 0,
        }
    }

        
    pub fn range(&self) -> (usize, usize) {
        let (first, last) = self.first_last();
        (first.start, last.end)
    }
        
    /// the length of the match, in bytes. This means it can be used to extract the unicode string from the string element
    pub fn match_len(&self) -> usize {
        let (start, end) = self.range();
        end - start
    }
    
    /// gets the subset of the target string matched by this **Path**
    pub fn matched_string(&'a self) -> &'a str {
        let (first, last) = self.first_last();
        &first.full_string[first.start..last.end]
    }

    /// returns ths **Limit** object for the Path
    pub fn limits(&self) -> Limits {
        match self {
            Path::Chars(steps) =>    steps[0].node.limits,
            Path::Set(steps) =>      steps[0].node.limits,
            Path::And(steps) =>      steps[0].node.limits,
            Path::Or(steps) =>       steps[0].node.limits,
            Path::None =>            panic!("Accessign limits() of None node"),
        }
    }

    // intended for internal use
    fn first_last(&self) -> (&Matched, &Matched) {
        match self {
            Path::Chars(steps) =>    (&steps[0].matched, &steps.last().unwrap().matched),
            Path::Set(steps) =>      (&steps[0].matched, &steps.last().unwrap().matched),
            Path::And(steps) =>      (&steps[0].matched, &steps.last().unwrap().matched),
            Path::Or(steps) =>       (&steps[0].matched, &steps.last().unwrap().matched),
            Path::None => panic!("NONE unexpected"),
        }
    }
    
    /// backs off a step on the path: when a path fails the process backtracks a step and tries again. This
    /// actually handles both the lazy and the greedy cases: for greedy the initial path walks to the upper limit
    /// or to failure, whichever is smaller, and backs off by removing a step from the path. For the lazy case
    /// the initial path is the minimum length, and it case of failure it adds another step to the end, up to the
    /// maximum limit
    fn pop(&mut self) -> Option<isize> {
        // for AND node first try popping off the last step in the last successful subPath . If that cannot be done then go ahead and pop pff the AND
        match self {
            Path::And(steps) => {
                let last = steps.len() - 1;
                let children = &mut steps[last].child_paths;
                let len = children.len();
                if !children.is_empty() {
                    if let Some(size) = children[len - 1].pop() {
                        return Some(size);
                    }
                }
            },
            Path::Or(steps) => {
                let last = steps.len() - 1;
                if let Some(size) = steps[last].child_path.pop() {
                    return Some(size);
                }
            },
            _ => (),
        }
        let ret = if self.limits().lazy { self.lazy_pop() }
        else { self.greedy_pop() };
        if trace(4) {
            trace_indent();
            println!("backoff: {:?}, success: {}", self, !ret.is_none())}
        ret
    }

    /// implementation of pop() for greedy evaluation
    fn greedy_pop(&mut self) -> Option<isize> {
        let limits = self.limits();
        let match_len = match self {
            Path::Chars(steps) => {
                let x = steps.pop().unwrap();
                if trace(5) {trace_indent(); println!("popping off {:?}", x); }
                x.matched.len()
            },
            Path::Set(steps) => {
                let x = steps.pop().unwrap();
                if trace(5) {trace_indent(); println!("popping off {:?}", x); }
                x.matched.len()
            },
            Path::And(steps) => {
                let x = steps.pop().unwrap();
                if trace(5) {trace_indent(); println!("popping off {:?}", x); }
                x.matched.len()
            },
            Path::Or(steps) => {
                let x = steps.pop().unwrap();
                if trace(5) {trace_indent(); println!("popping off {:?}", x); }
                x.matched.len()
            },
            Path::None => panic!("pop(): NONE unexpected"),
        };
        if limits.check(self.len()) == 0 { Some(match_len as isize) } else { None }
    }

    /// implementation of pop() for lazy evaluation
    fn lazy_pop(&mut self) -> Option<isize> {
        if self.limits().check(self.len() + 1) != 0 { return None; }
        let mut match_len: isize = -1;
        match self {
            Path::Chars(steps) => {
                if let Some(step) = steps[steps.len() - 1].step() {
                    match_len = step.matched.len() as isize;
                    steps.push(step);
                }
            },
            Path::Set(steps) => {
                if let Some(step) = steps[steps.len() - 1].step() {
                    match_len = step.matched.len() as isize;
                    steps.push(step);
                }
            },
            Path::And(steps) => {
                let len = steps.len();
                if let Some(step) = steps[len - 1].step() {
                    match_len = step.matched.len() as isize;
                    steps.push(step);
                }
            },
            Path::Or(steps) => {
                if let Some(step) = steps[steps.len() - 1].step() {
                    match_len = step.matched.len() as isize;
                    steps.push(step);
                }
            },
            Path::None => panic!("NONE unexpected"),
        }
        if match_len < 0 { None }
        else { Some(-match_len) }
    }

    /// recursively creates **Report** objects for a path. For the branches (And and Or) this means recording itself
    /// and then collecting from the children recursively. leaves just need to record themselves
    pub fn gather_reports(&'a self, char_start: usize) -> (Vec<Report>, usize) {
        let mut char_pos = char_start;
        let mut reports = Vec::<Report>::new();
        match self {
            Path::And(steps) => {
                let mut iter = steps.iter();
                // There is an entry for 0 in the list, if there are others pop them off
                if steps.len() > 1 { let _ = iter.next(); }
                for step in iter {
                    let (mut subreport, pos) = step.make_report(char_pos);
                    char_pos = pos;
                    if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                    else { reports.push(subreport); }
                }
            },
            Path::Or(steps) => {
                let mut iter = steps.iter();
                // There is an entry for 0 in the list, if there are others pop them off
                if steps.len() > 1 { let _ = iter.next(); }
                for step in iter {
                    let (mut subreport, pos) = step.make_report(char_pos);
                    char_pos = pos;
                    if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                    else { reports.push(subreport); }
                }
            },
            Path::Chars(steps) => {
                let mut iter = steps.iter();
                // There is an entry for 0 in the list, if there are others pop them off
                if steps.len() > 1 { let _ = iter.next(); }
                for step in iter {
                    let (subreport, pos) = step.make_report(char_pos);
                    char_pos = pos;
                    if !subreport.name.is_none() { reports.push(subreport); }
                }
            },
            Path::Set(steps) => {
                let mut iter = steps.iter();
                // There is an entry for 0 in the list, if there are others pop them off
                if steps.len() > 1 { let _ = iter.next(); }
                for step in iter {
                    let (subreport, pos) = step.make_report(char_pos);
                    char_pos = pos;
                    if !subreport.name.is_none() { reports.push(subreport); }
                }
            },
            Path::None => panic!("Should not be any None path when compiling report"),
        }
        (reports, char_pos)
    }
}

/// prints message when entering walk (trace level 2)
fn trace_start_walk<'a, T: Debug>(vec: &'a Vec<T>) {
    if trace(2) {
        trace_indent();
        println!("Start walk for {:?}", &vec[0]);
        trace_change_indent(1);
    }
}

/// prints message when finishing walk (trace level 3)
fn trace_end_walk(path: Path) -> Path {
    if trace(2) {
        trace_change_indent(-1);
        if trace(3) {
            trace_indent();
            let ok = if path.limits().check(path.len()) == 0 { format!("matches \"{}\"", path.matched_string())} else { "no match".to_string()};
            println!("End walk for {:?}, {} steps, {}", path, path.len() - 1, ok);
        }
    }
    path
}

/// prints message when taking a new step during a walk (trace level 4)
fn trace_pushing<'a, T: Debug>(obj: &'a T, len: usize) {
    if trace(4) {
        trace_indent();
        println!("Pushing {:?} rep {}", obj, len - 1);
    }
}


//////////////////////////////////////////////////////////////////
//
// Debug implementations: used for tracing
//
//////////////////////////////////////////////////////////////////
impl<'a> Walker for CharsStep<'a> {}
impl<'a> Walker for SetStep<'a> {}
impl<'a> Walker for AndStep<'a> {}
impl<'a> Walker for OrStep<'a> {}

impl<'a> Debug for CharsStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, string {} len {}", self.node, self.matched.abbrev(), self.matched.len())
    }
}

impl<'a> Debug for SetStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, string {} len {}", self.node, self.matched.abbrev(), self.matched.len())
    }
}

impl<'a> Debug for AndStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut child_counts: String = "".to_string();
        for p in self.child_paths.iter() { child_counts.push_str(&format!("{}, ", p.len())); }
        for _i in self.child_paths.len()..self.node.nodes.len() { child_counts.push_str("-, "); }
        write!(f, "{{{:?}}} state [{}], string {} len {}",
               self.node,
               child_counts,
               self.matched.abbrev(),
               self.matched.len())
    }
}

impl<'a> Debug for OrStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, branch {}, branch reps {}, string {} len {}",
               self.node,
               self.which,
               self.child_path.len(),
               self.matched.abbrev(),
               self.matched.len())
    }
}

impl<'a> Debug for Path<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Path::Chars(steps) =>    {
                if steps.is_empty() { write!(f, "Path Chars, reps 0, match \"\"") }
                else {write!(f, "Path {:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())}
            },
            Path::Set(steps) => {
                if steps.is_empty() { write!(f, "Path Set, reps 0, match \"\"") }
                else {write!(f, "Path {:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())}
            },
            Path::And(steps) => {
                if steps.is_empty() { write!(f, "Path And, reps 0, match \"\"") }
                else {
                    write!(f, "Path {:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())
                }
            },
            Path::Or(steps) => {
                if steps.is_empty() { write!(f, "Path Or, reps 0, match \"\"") }
                else {
                    write!(f, "Path {:?}, reps {}, match \"{}\"", steps[steps.len() - 1], self.len(), self.matched_string())
                }
            },
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
    pub fn walk(node: &'a CharsNode, matched: Matched<'a>) -> Path<'a> {
        let mut steps = vec![CharsStep {node, matched}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            match steps.last().unwrap().step() {
                Some(s) => {
                    steps.push(s);
                    trace_pushing::<CharsStep>(steps.last().unwrap(), steps.len());
                },
                None => break,
            }
        }
        trace_end_walk(Path::Chars(steps))
    }

    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    /// try to take a single step over a string of regular characters
    fn step(&self) -> Option<CharsStep<'a>> {
        let mut step = CharsStep {node: self.node, matched: self.matched.next(0)};
        if step.matched.end == step.matched.full_string.len() { return None; }
        if let Some(size) = step.node.matches(step.matched.unterminated()) {
            step.matched.move_end(size as isize);
            Some(step)
        } else { None }
    }

    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&self, char_start: usize) -> (Report, usize) {
        let matched_string = &self.matched.string();
        let char_len = self.matched.len_chars();
        (Report {found: matched_string.to_string(),
                 name: self.node.named.clone(),
                 pos: (char_start, char_start + char_len),
                 bytes: (self.matched.start, self.matched.end),
                 subreports: Vec::<Report>::new()},
                 char_start + char_len)
    }
}

impl<'a> SetStep<'a> {
    /// start a Path using a set to match, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a SetNode, matched: Matched<'a>) -> Path<'a> {
        let mut steps = vec![SetStep {node, matched}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            match steps.last().unwrap().step() {
                Some(s) => {
                    trace_pushing::<SetStep>(&s, steps.len());
                    steps.push(s);
                },
                None => { break; }
            }
        }
        trace_end_walk(Path::Set(steps))
    }
    /// try to take a single step over a set of characters
    fn step(&self) -> Option<SetStep<'a>> {
        if let Some(len) = self.node.matches(self.matched.remainder()) {
            Some(SetStep {node: self.node, matched: self.matched.next(len as usize)})
        } else {
            None
        }
    }

    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&self, char_start: usize) -> (Report, usize) {
        let matched_string = self.matched.string();
        let char_len = self.matched.len_chars();
        (Report {found: matched_string.to_string(),
                 name: self.node.named.clone(),
                 pos: (char_start, char_start + char_len),
                 bytes: (self.matched.start, self.matched.end),
                 subreports: Vec::<Report>::new()},
         char_start + char_len)
    }
}

impl<'a> AndStep<'a> {
    /// start a Path using an And node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a AndNode, matched: Matched<'a>) -> Path<'a> {
        let mut steps = vec![AndStep {node, matched, child_paths: Vec::<Path<'a>>::new()}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            match steps.last().unwrap().step() {
                Some(s) => {
                    trace_pushing::<AndStep>(&s, steps.len());
                    steps.push(s);
                },
                None => { break; }
            }
        }
        trace_end_walk(Path::And(steps))
    }

    /// try to take a single step matching an And node
    fn step(&self) -> Option<AndStep<'a>> {
        let mut step = AndStep {node: self.node,
                                matched: self.matched.next(0),
                                child_paths: Vec::<Path<'a>>::new(),
        };
        loop {
            let child_len = step.child_paths.len();
            if child_len == step.node.nodes.len() { break; }
            let child_path = step.node.nodes[child_len].walk(step.matched.next(0));
            if child_path.limits().check(child_path.len()) == 0 {
                step.matched.end += child_path.match_len();
                step.child_paths.push(child_path);
                if trace(4) {
                    trace_change_indent(-1);
                    trace_indent();
                    println!("-- new step in AND: {:?}", step);
                    trace_change_indent(1);
                }
            } else if step.back_off() {
                let (_, end) = step.child_paths.last().unwrap().range();
                step.matched.end = end;
            } else {
                return None;
            }
        }
        let (_, end) = step.child_paths.last().unwrap().range();
        step.matched.set_end(end);
        Some(step)
    }

    /// back off a step after a failed match: after taking a step back see if the path length still falls within
    /// the desired limits, if it does continue along that new path to see if it succeeds, if not remove that child
    /// and then back off on the preceding child. When all children have been exhausted the step has failed.
    fn back_off(&mut self) -> bool {
        loop {
            if self.child_paths.is_empty() { return false; }
            let last_pathnum = self.child_paths.len() - 1;
            if let Some(size) = self.child_paths[last_pathnum].pop() {
                // self.matched.end = (self.matched.end as isize - size) as usize;
                self.matched.move_end(-size);
                return true;
            }
            if let Some(path) = self.child_paths.pop() {
                if self.child_paths.is_empty() {
                    self.matched.end -= path.matched_string().len();
                    continue;
                }
            } return false;
        }
    }
    
    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&self, char_start: usize) -> (Report, usize) {
//        println!("reporting on {:#?}", self);
        let mut reports = Vec::<Report>::new();
        let mut char_end = char_start;
        for p in &self.child_paths {
            let (mut subreports, loc) = p.gather_reports(char_end);
            char_end = loc;
            reports.append(&mut subreports);
        }
        (Report {found: self.matched.string().to_string(),
                 name: self.node.named.clone(),
                 pos: (char_start, char_end),
                 bytes: (self.matched.start, self.matched.end),
                 subreports: reports},
         char_end)
    }
}

/// OR does not have a *step()* function because it cannot have a repeat count (to repeat an OR it must be enclosed in an AND)
impl<'a> OrStep<'a> {
    
    /// start a Path using an And node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a OrNode, matched: Matched<'a>) -> Path<'a> {
        let mut steps = vec![OrStep {node, matched, child_path: Box::new(Path::None), which: 0}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            if let Some(s) = steps.last().unwrap().step() {
                trace_pushing::<OrStep>(&s, steps.len());
                steps.push(s);
            } else { break; }
        }
        trace_end_walk(Path::Or(steps))
    }
    
    /// try to take a single step matching an Or node
    fn step(&self) -> Option<OrStep<'a>> {
        let mut step = OrStep {node: self.node,
                               matched: self.matched.next(0),
                               which: 0, 
                               child_path: Box::new(Path::None),
        };
        loop {
            if step.which == step.node.nodes.len() { return None; }
            step.child_path = Box::new(step.node.nodes[step.which].walk(self.matched.next(0)));
            //step.matched.end = step.matched.start + step.child_path.match_len();
            step.matched.set_end(step.matched.start + step.child_path.match_len());
            if step.child_path.limits().check(step.child_path.len()) == 0 { break; }
            if !step.back_off() { return None; }
        }
        if trace(4) {
            trace_change_indent(-1);
            trace_indent();
            println!("new OR step: {:?}", step);
            trace_change_indent(1);
        }
//        println!("xxxx {:#?}", step.child_path);
        Some(step)
    }

    // return value:
    // - 0: failed
    // - 1: branch failed, try the next one
    // - 2: succeeded
    fn back_off(&mut self) -> bool {
        if let Some(len) = self.child_path.pop() {
            //self.matched.end = (self.matched.end as isize - len) as usize;
            self.matched.move_end(-len);
            true
        } else {
            // self.matched.end = self.matched.start;
            self.matched.set_end(self.matched.start);
            self.which += 1;
            self.which < self.node.nodes.len()
        }
    }

    /// Compiles a **Report** object from this path and its child after a successful search
    fn make_report(&self, char_start: usize) -> (Report, usize) {
        let (subreports, char_end) = self.child_path.gather_reports(char_start);
        (Report {found: self.matched.string().to_string(),
                 name: self.node.named.clone(),
                 pos: (char_start, char_end),
                 bytes: (self.matched.start, self.matched.end),
                 subreports},
         char_end)
    }
}

impl<'a> Clone for Matched<'a> {
    fn clone(&self) -> Self { Matched {full_string: &self.full_string[..], start: self.start, end: self.end, char_start: self.char_start} }
}

impl<'a> Matched<'a> {
    /// Returns the length of the match in bytes
    fn len(&self) -> usize { self.end - self.start }
    /// Returns the length of the match in chars
    fn len_chars(&self) -> usize { self.string().chars().count() }
    /// Returns pointer to the matching str slice
    fn string(&self) -> &str { &self.full_string[self.start..self.end] }
    /// returns the remainder of the str starting from the match beginning
    fn unterminated(&self) -> &str { &self.full_string[self.start..] }
    /// returns the remainder of the str starting from the match end
    fn remainder(&self) -> &str { &self.full_string[self.end..] }
    /// Builds a new Matched object immediately following the one pointed to by self
    fn next(&self, len: usize) -> Matched<'a> {
        Matched {full_string: &self.full_string[..], start: self.end, end: self.end + len, char_start: self.char_start + self.len_chars() }
    }
    /// Moves the end of Matched by the amount given
    fn move_end(&mut self, delta: isize) {
        if delta > 0 { self.set_end(self.end + delta as usize); } else { self.set_end(self.end - (-delta) as usize); }
    }
    /// Moves the end of Matched to the new position
    fn set_end(&mut self, new_end: usize) {
        self.end = new_end;
    }

    /// **abbrev()** is used in the pretty-print of steps: The display prints out the string at the start of the step. If it is too
    /// long it distracts from the other output. **abbrev** limits the length of the displayed string by replacing its end with "..."
    fn abbrev(&self) -> String {
        let s = &self.full_string[self.start..];
        let dots = if self.len() < unsafe { ABBREV_LEN } {""} else {"..."};
        format!("\"{}\"{}", s, dots)
    }
}

// TODO: get rid of unsafe
/// Set the length a String can be before it is abbreviated
pub fn set_abbrev_size(size: u32) { unsafe {ABBREV_LEN = size as usize; }}
static mut ABBREV_LEN: usize = 5;
