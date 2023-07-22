//! ## Regular expression search: Tree walker
//! This module contains the code for walking the tree to find a RE match. Each match of a **Node** (representing a unit in the
//! RE tree) is represented by a **Step** object. The **Step**s are grouped in vectors to form **Path**s, each of which represents
//! a walk through the tree. When a **Path** reaches the end of the tree successfully it means the search has succeeded and that
//! **Path* is returned, representing a matched string, so it can generate a **Report** giving its route.
use super::*;
use crate::{trace, trace_indent, trace_change_indent};
use std::io::BufReader;
use std::io::BufRead;
//use lazy_static::lazy_static;
use std::sync::Mutex;
use once_cell::sync::Lazy;

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

/// Represents a single step for a CharsNode (a string of regular characters)
pub struct CharsStep<'a> {
    /// The node from phase 1
    node: &'a CharsNode,
    matched: Matched,
}

/// Represents a single step for a SpecialNode (a special character)
pub struct SpecialStep<'a> {
    /// The node from phase 1
    node: &'a SpecialNode,
    matched: Matched,
}

/// Represents a single step for a RangeNode (ie [abcx-z])
pub struct RangeStep<'a> {
    /// The node from phase 1
    node: &'a RangeNode,
    matched: Matched,
}

/// Represents a single step for an AndNode (a collection of 0 or more nodes that all must match)
pub struct AndStep<'a> {
    /// The node from phase 1
    node: &'a AndNode,
    matched: Matched,
    /// A vector of Paths saving the current state of this And node. Each entry is a **Path** based on the **nodes** member of the **AndNode** structure.
    /// When the Paths vector is filled this step for the And node has succeeded.
    child_paths: Vec<Path<'a>>,
}

/// Represents a single step for an OrNode (a collection of 0 or more nodes that one must match)
pub struct OrStep<'a> {
    /// The node from phase 1
    node: &'a OrNode,
    matched: Matched,
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
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>),  Special(Vec<SpecialStep<'a>>), Range(Vec<RangeStep<'a>>), And(Vec<AndStep<'a>>), Or(Vec<OrStep<'a>>), None }

impl<'a> Path<'a> {
    // basic methods
    pub fn is_none(&self) -> bool { matches!(self, Path::None)}
    
    pub fn is_empty(&self) -> bool { self.len() == 0 }
    
    /// length of the **Path** (the number of **Step**s it has)
    pub fn len(&self) -> usize {
        match self {
            Path::Chars(steps) => steps.len(),
            Path::Special(steps) => steps.len(),
            Path::Range(steps) => steps.len(),
            Path::And(steps)   => steps.len(),
            Path::Or(steps)    => steps.len(),
            Path::None => 0,
        }
    }

    /// gets the range of the path, using bytes
    pub fn range(&self) -> (usize, usize) {
        let (first, last) = self.first_last();
        (first.start, last.end)
    }

    /// gets the byte count of the end of the path
    pub fn end(&self) -> usize {
        let (_, last) = self.first_last();
        last.end
    }
        
    /// the length of the path, in bytes. This means it can be used to extract the unicode string from the string element
    pub fn match_len(&self) -> usize {
        let (start, end) = self.range();
        end - start
    }
    
    /// gets the subset of the target string matched by this **Path**
    /// For debug use: allocates String
    pub fn matched_string(&'a self) -> String {
        let (first, last) = self.first_last();
        Input::apply(|input| input.full_text[first.start..last.end].to_string())
    }

    /// returns ths **Limit** object for the Path
    pub fn limits(&self) -> Limits {
        match self {
            Path::Chars(steps) => steps[0].node.limits,
            Path::Special(steps) => steps[0].node.limits,
            Path::Range(steps) => steps[0].node.limits,
            Path::And(steps) => steps[0].node.limits,
            Path::Or(steps) => steps[0].node.limits,
            Path::None => panic!("Accessing limits() of None node"),
        }
    }

    // intended for internal use, gets the first and last steps in the path
    fn first_last(&self) -> (&Matched, &Matched) {
        match self {
            Path::Chars(steps) =>    (&steps[0].matched, &steps.last().unwrap().matched),
            Path::Special(steps) =>    (&steps[0].matched, &steps.last().unwrap().matched),
            Path::Range(steps) =>    (&steps[0].matched, &steps.last().unwrap().matched),
            Path::And(steps) =>      (&steps[0].matched, &steps.last().unwrap().matched),
            Path::Or(steps) =>       (&steps[0].matched, &steps.last().unwrap().matched),
            Path::None => panic!("NONE unexpected"),
        }
    }

    /// used to back off a failed repeated match to see if thte path can be saved. There are 3 steps in
    /// backing off. The actual implementation depends on whether it is using lazy or greedy match algorithm.
    ///
    /// - back off child path (for **AND** and **OR**): if a child path can be backed off try that
    /// - change subpaths (for **AND** and **OR**) if a path becomes too sort (or too long, for lazy) pop that
    ///   subpath off and check. For **AND** this means backing off on the previous subpath, for **OR** it means
    ///   going to the next option
    /// - back off the last step, check if that still meets the requirements. For greedy evaluation this means popping
    ///   off a step from the Path, for lazy eval it means adding a new step
    fn back_off(&mut self) -> Result<bool, Error> {
        if trace(6) { trace_change_indent(1); }
        let limits = self.limits();
        let mut ret = false;
        if limits.lazy() {
            match self {
                Path::Chars(steps) => {
                    if limits.check(steps.len() + 1) == 0 {
                        if let Some(next_step) = steps.last().unwrap().step() {
                            steps.push(next_step);
                            ret = true;
                        }
                    }
                    if trace(6) { trace_indent(); println!("back off Path lazy: {:?}, new step count {}: {}", steps.last().unwrap(), steps.len(), ret); }
                },
                Path::Special(steps) => {
                    if limits.check(steps.len() + 1) == 0 {
                        if let Some(next_step) = steps.last().unwrap().step() {
                            steps.push(next_step);
                            ret = true;
                        }
                    }
                    if trace(6) { trace_indent(); println!("back off Path lazy: {:?}, new step count {}: {}", steps.last().unwrap(), steps.len(), ret); }
                },
                Path::Range(steps) => {
                    if limits.check(steps.len() + 1) == 0 {
                        if let Some(next_step) = steps.last().unwrap().step() {
                            steps.push(next_step);
                            ret = true;
                        }
                    }
                    if trace(6) { trace_indent(); println!("back off Path lazy: {:?}, new step count {}: {}", steps.last().unwrap(), steps.len(), ret); }
                },
                Path::And(steps) => {
                    let len0 = steps.len();
                    if steps[len0 - 1].back_off()? { ret = true; }
                    else if limits.check(steps.len() + 1) == 0 {
                        if let Some(next_step) = steps[len0 - 1].step()? {
                            steps.push(next_step);
                            ret = true;
                        } 
                    } 
                    if trace(6) { trace_indent(); println!("back off Path lazy: {:?}, steps was {}, now {}: {}", steps.last().unwrap(), len0, steps.len(), ret); }
                },
                Path::Or(steps) => {
                    let len0 = steps.len();
                    if steps[len0 - 1].back_off()? { ret = true; }
                    else if limits.check(steps.len() + 1) == 0 {
                        if let Some(next_step) = steps[len0 - 1].step()? {
                            steps.push(next_step);
                            ret = true;
                        } 
                    } 
                    if trace(6) { trace_indent(); println!("back off Path lazy: {:?}, steps was {}, now{}: {}", steps.last().unwrap(), len0, steps.len(), ret); }
                },
                _ => panic!("Should not be trying to back off None node"),
            }
        } else {
            match self {
                Path::Chars(steps) => {
                    let _last_step = steps.pop().unwrap();
                    ret = limits.check(steps.len()) == 0;
                    if trace(6) { trace_indent(); println!("back off Path: {:?}, new step count {}: {}", steps.last(), steps.len(), ret); }
                },
                Path::Special(steps) => {
                    let _last_step = steps.pop().unwrap();
                    ret = limits.check(steps.len()) == 0;
                    if trace(6) { trace_indent(); println!("back off Path: {:?}, new step count {}: {}", steps.last(), steps.len(), ret); }
                },
                Path::Range(steps) => {
                    let _last_step = steps.pop().unwrap();
                    ret = limits.check(steps.len()) == 0;
                    if trace(6) { trace_indent(); println!("back off Path: {:?}, new step count {}: {}", steps.last(), steps.len(), ret); }
                },
                Path::And(steps) => {
                    let mut last_step = steps.pop().unwrap();
                    let len0 = steps.len();
                    if last_step.back_off()? {
                        ret = true;
                        steps.push(last_step);
                    } else {
                        ret = limits.check(steps.len()) == 0;
                    }
                    if trace(6) { trace_indent(); println!("back off: {:?}, steps was {}, now {}: {}", steps.last().unwrap(), len0, steps.len(), ret); }
                },
                Path::Or(steps) => {
                    let len0 = steps.len();
                    let mut last_step = steps.pop().unwrap();
                    if last_step.back_off()? {
                        ret = true;
                        steps.push(last_step);
                    } else {
                        ret = limits.check(steps.len()) == 0;
                    }
                    if trace(6) { trace_indent(); println!("back off Path: {:?} steps was {}, now {}", steps.last().unwrap(), len0, steps.len()); }
                },
                _ => panic!("Should not be trying to back off None node"),
            }
        }
        if trace(6) { trace_change_indent(-1); }
        Ok(ret)
    }

    /// recursively creates **Report** objects for a path. For the branches (And and Or) this means recording itself
    /// and then collecting from the children recursively. leaves just need to record themselves
    pub fn gather_reports(&'a self) -> Vec<Report> {
        let mut reports = Vec::<Report>::new();
        match self {
            Path::And(steps) => {
                // The first step represents 0 matches, it should be skipped if there are any others
                for step in steps.iter().skip(if steps.len() > 1 {1} else {0}) {
                    let mut subreport = step.make_report();
                    if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                    else { reports.push(subreport); }
                }
                if steps[0].node.name_outside {
                    let mut matched = steps[0].matched;
                    matched.end = steps.last().unwrap().matched.end;
                    let mut subreports = Vec::new();
                    reports.into_iter().rev().for_each(|mut subs| subreports.append(&mut subs.subreports));
                    reports = vec![Report { matched, name: steps[0].node.named.clone(), subreports }];
                };
            },
            Path::Or(steps) => {
                for step in steps.iter().skip(if steps.len() > 1 {1} else {0}) {
                    let mut subreport = step.make_report();
                    if subreport.name.is_none() { reports.append(&mut subreport.subreports); }
                    else { reports.push(subreport); }
                }
                if steps[0].node.name_outside {
                    let mut matched = steps[0].matched;
                    matched.end = steps.last().unwrap().matched.end;
                    let mut subreports = Vec::new();
                    reports.into_iter().rev().for_each(|mut subs| subreports.append(&mut subs.subreports));
                    reports = vec![Report { matched, name: steps[0].node.named.clone(), subreports }];
                };
            },
            Path::Chars(steps) => {
                for step in steps.iter().skip(if steps.len() > 1 {1} else {0}) {
                    let subreport = step.make_report();
                    if subreport.name.is_some() { reports.push(subreport); }
                }
                if steps[0].node.name_outside {
                    let mut matched = steps[0].matched;
                    matched.end = steps.last().unwrap().matched.end;
                    let mut subreports = Vec::new();
                    reports.into_iter().rev().for_each(|mut subs| subreports.append(&mut subs.subreports));
                    reports = vec![Report { matched, name: steps[0].node.named.clone(), subreports }];
                }
            },
            Path::Special(steps) => {
                for step in steps.iter().skip(if steps.len() > 1 {1} else {0}) {
                    let subreport = step.make_report();
                    if subreport.name.is_some() { reports.push(subreport); }
                }
                if steps[0].node.name_outside {
                    let mut matched = steps[0].matched;
                    matched.end = steps.last().unwrap().matched.end;
                    let mut subreports = Vec::new();
                    reports.into_iter().rev().for_each(|mut subs| subreports.append(&mut subs.subreports));
                    reports = vec![Report { matched, name: steps[0].node.named.clone(), subreports }];
                };
            },
            Path::Range(steps) => {
                for step in steps.iter().skip(if steps.len() > 1 {1} else {0}) {
                    let subreport = step.make_report();
                    if subreport.name.is_some() { reports.push(subreport); }
                }
                if steps[0].node.name_outside {
                    let mut matched = steps[0].matched;
                    matched.end = steps.last().unwrap().matched.end;
                    let mut subreports = Vec::new();
                    reports.into_iter().rev().for_each(|mut subs| subreports.append(&mut subs.subreports));
                    reports = vec![Report { matched, name: steps[0].node.named.clone(), subreports }];
                };
            }
            Path::None => panic!("Should not be any None path when compiling report"),
        }
        reports
    }

    /// pretty prints a report using indentation to show inclusion
    pub fn dump(&self, mut indent: usize) {
        if indent == 0 { trace_indent(); println!("PATH {} ------------", self.node_type()); indent = 1;}
        match self {
            //Path::Chars(steps) => { for i in 0..steps.len() {steps[i].dump(i, indent)}},
            Path::Chars(steps) => steps.iter().enumerate().for_each(|x| x.1.dump(x.0, indent)),
            Path::Special(steps) => steps.iter().enumerate().for_each(|x| x.1.dump(x.0, indent)),
            Path::Range(steps) =>  steps.iter().enumerate().for_each(|x| x.1.dump(x.0, indent)),
            Path::And(steps) => steps.iter().enumerate().for_each(|x| x.1.dump(x.0, indent)),
            Path::Or(steps) => steps.iter().enumerate().for_each(|x| x.1.dump(x.0, indent)),
            Path::None => { trace_indent(); println!("|{0:1$}0: NONE \"\"", "", 4*indent, )},
        }
        if indent == 1 { trace_indent(); println!("PATH {} ------------", self.node_type()); }
    }
    pub fn node_type(&self) -> &str {
        match self {
            Path::Chars(_) => "Chars",
            Path::Special(_) => "Special", 
            Path::Range(_) => "Range",
            Path::And(_) => "And",
            Path::Or(_) => "Or",
            Path::None => "None",
        }
    }
}

// TODO: This should have a Result<> return, but it is a late addition and I don't really have the incentive to
// change all the return types now
// I think new steps cannot go backwards, so if any step but step 0 has a series of matches of length 0 (maybe even
// 2) it means an infinite loop. I chose to call this every 30 steps, but if the max level is set less than max
// I assume the caller knows what he is doing
/// Call to check whether the parser is in an infinite loop. Currently panics if it is, I need to change the call stack
/// to return and catch an error
fn loop_check(&matched: &Matched, limits: &Limits) -> Result<(), Error> {
    if matched.len_bytes() == 0 && limits.max == EFFECTIVELY_INFINITE {
        Err(Error::make(200, "Appears to be an infinite loop"))
    } else { Ok(()) }
}
    
/// Experimental: I want to use this to simplify the **impl Path ** code. It is begun but not implemented yet
///
trait Walker<'a> {
    fn make_report(&'a self) -> Report;
    fn name_details(&self) -> (&Option<String>, bool);
    fn get_matched(&self) -> Matched;
}

/// Trace Levels
/// level 1: just trace phase
/// level 2: trace start of walks
/// level 3: trace start and  end of walks
/// level 4: trace start and  end of walks and each new child in an AND
/// level 10: dump out paths as they are extended
/// prints message when entering walk (trace level 2)
fn trace_start_walk<T: Debug>(vec: &[T]) {
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
        if trace(10) { path.dump(0); }
    }
    path
}

/// prints message when adding a Step to a Path (trace level 4)
fn trace_pushing<T: Debug>(obj: &T, len: usize) {
    if trace(4) {
        trace_indent();
        println!("Pushing {:?} rep {}", obj, len - 1);
    }
}
/// prints message when taking a new step during a walk (trace level 4)
fn trace_new_step(and_step: &AndStep) {
    if trace(5) {
        trace_indent();
        println!("-- new child step in AND: {:?}", and_step);
    }
}


//////////////////////////////////////////////////////////////////
//
// Debug implementations: used for tracing
//
//////////////////////////////////////////////////////////////////
impl<'a> Debug for CharsStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, {:?}", self.node, self.matched)
    }
}

impl<'a> Debug for SpecialStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, {:?}", self.node, self.matched)
    }
}

impl<'a> Debug for RangeStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, {:?}", self.node, self.matched)
    }
}

impl<'a> Debug for AndStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut child_counts: String = "".to_string();
        for p in self.child_paths.iter() { child_counts.push_str(&format!("{}, ", p.len())); }
        for _i in self.child_paths.len()..self.node.nodes.len() { child_counts.push_str("-, "); }
        write!(f, "{{{:?}}} state [{}], {:?}",
               self.node,
               child_counts,
               self.matched)
    }
}

impl<'a> Debug for OrStep<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{{{:?}}}, branch {} of {}, branch reps {}, {:?} ",
               self.node,
               self.which + 1,
               self.node.nodes.len(),
               self.child_path.len(),
               self.matched
               )
    }
}

impl<'a> Debug for Path<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Path::Chars(steps) =>    {
                if steps.is_empty() { write!(f, "Path Chars, reps 0, match \"\"") }
                else {write!(f, "Path {:?}, steps {}, match \"{}\"", steps[steps.len() - 1].node, self.len(), self.matched_string())}
            },
            Path::Special(steps) =>    {
                if steps.is_empty() { write!(f, "Path Special, reps 0, match \"\"") }
                else {write!(f, "Path {:?}, steps {}, match \"{}\"", steps[steps.len() - 1].node, self.len(), self.matched_string())}
            },
            Path::Range(steps) =>    {
                if steps.is_empty() { write!(f, "Path Chars, reps 0, match \"\"") }
                else {write!(f, "Path {:?}, steps {}, match \"{}\"", steps[steps.len() - 1].node, self.len(), self.matched_string())}
            },
            Path::And(steps) => {
                if steps.is_empty() { write!(f, "Path And, reps 0, match \"\"") }
                else {
                    write!(f, "Path {:?}, steps {}, match \"{}\"", steps[steps.len() - 1].node, self.len(), self.matched_string())
                }
            },
            Path::Or(steps) => {
                if steps.is_empty() { write!(f, "Path Or, reps 0, match \"\"") }
                else {
                    write!(f, "Path {:?}, steps {}, match \"{}\"", steps[steps.len() - 1].node, self.len(), self.matched_string())
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

impl<'a> Walker<'a> for CharsStep<'a> {
    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&'a self) -> Report {
        Report {matched: self.matched,
                 name: self.node.named.clone(),
                 subreports: Vec::<Report>::new()}
    }
    fn name_details(&self) -> (&Option<String>, bool) { (&self.node.named, self.node.name_outside) }
    fn get_matched(&self) -> Matched { self.matched }
}

// Any way to make walk() generic?
impl<'a> CharsStep<'a> {
    /// start a Path using a string of chars, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a CharsNode, matched: Matched) -> Result<Path<'a>, Error> {
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
        Ok(trace_end_walk(Path::Chars(steps)))
    }

    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    /// try to take a single step over a string of regular characters
    fn step(&self) -> Option<CharsStep<'a>> {
        let mut step = CharsStep {node: self.node, matched: self.matched.next(0)};
        Input::extend_quiet(step.matched.start + self.node.string.len() + 1);
        if step.matched.end == Input::len() { return None; }

        if let Some(size) = Input::apply(|input| { step.node.matches(&input.full_text[step.matched.start..])}) {
            step.matched.move_end(size as isize);
            Some(step)
        } else { None }
    }
    fn dump(&self, rank: usize, indent: usize) {
        trace_indent(); println!("|{0:1$}{2}: {3:?}", "", 4*indent, rank, self);
    }
}

impl<'a> Walker<'a> for SpecialStep<'a> {
    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&'a self) -> Report {
        Report {matched: self.matched,
                name: self.node.named.clone(),
                subreports: Vec::<Report>::new()}
    }
    fn name_details(&self) -> (&Option<String>, bool) { (&self.node.named, self.node.name_outside) }
    fn get_matched(&self) -> Matched { self.matched }
}

impl<'a> SpecialStep<'a> {
    /// start a Path using a string of chars, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a SpecialNode, matched: Matched) -> Result<Path<'a>, Error> {
        let mut steps = vec![SpecialStep {node, matched}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            match steps.last().unwrap().step() {
                Some(s) => {
                    steps.push(s);
                    trace_pushing::<SpecialStep>(steps.last().unwrap(), steps.len());
                },
                None => break,
            }
        }
        Ok(trace_end_walk(Path::Special(steps)))
    }

    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    /// try to take a single step over a string of regular characters
    fn step(&self) -> Option<SpecialStep<'a>> {
        let mut step = SpecialStep {node: self.node, matched: self.matched.next(0)};
        if let Some(size) = Input::apply(|input| { step.node.matches(&input.full_text[step.matched.start..])}) {
            step.matched.move_end(size as isize);
            Some(step)
        } else { None }
    }

    fn dump(&self, rank: usize, indent: usize) {
        trace_indent(); println!("|{0:1$}{2}: {3:?} ", "", 4*indent, rank, self,);
    }
}

impl<'a> Walker<'a> for RangeStep<'a> {
    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&'a self) -> Report {
        Report {matched: self.matched,
                name: self.node.named.clone(),
                subreports: Vec::<Report>::new()}
    }
    fn name_details(&self) -> (&Option<String>, bool) { (&self.node.named, self.node.name_outside) }
    fn get_matched(&self) -> Matched { self.matched }
}

impl<'a> RangeStep<'a> {
    /// start a Path using a string of chars, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a RangeNode, matched: Matched) -> Result<Path<'a>, Error> {
        let mut steps = vec![RangeStep {node, matched}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            match steps.last().unwrap().step() {
                Some(s) => {
                    steps.push(s);
                    trace_pushing::<RangeStep>(steps.last().unwrap(), steps.len());
                },
                None => break,
            }
        }
        Ok(trace_end_walk(Path::Range(steps)))
    }

    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    /// try to take a single step over a string of regular characters
    fn step(&self) -> Option<RangeStep<'a>> {
        let mut step = RangeStep {node: self.node, matched: self.matched.next(0)};
        if step.matched.end == Input::len() { return None; }
        
        if let Some(size) = Input::apply(|input| { step.node.matches(&input.full_text[step.matched.start..])}) {
            step.matched.move_end(size as isize);
            Some(step)
        } else { None }
    }

    fn dump(&self, rank: usize, indent: usize) {
        trace_indent(); println!("|{0:1$}{2}: {3:?}", "", 4*indent, rank, self);
    }
}

impl<'a> Walker<'a> for AndStep<'a> {
    /// Compiles a **Report** object from this path and its children after a successful search
    fn make_report(&'a self) -> Report {
        let mut reports = Vec::<Report>::new();
        for p in &self.child_paths {
            let mut subreports = p.gather_reports();
            reports.append(&mut subreports);
        }
        Report { matched: self.matched,
                 name: self.node.named.clone(),
                 subreports: reports}
    }
    fn name_details(&self) -> (&Option<String>, bool) { (&self.node.named, self.node.name_outside) }
    fn get_matched(&self) -> Matched { self.matched }
}

impl<'a> AndStep<'a> {
    /// start a Path using an And node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a AndNode, matched: Matched) -> Result<Path<'a>, Error> {
        let mut steps = vec![AndStep {node, matched, child_paths: Vec::<Path<'a>>::new()}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            let len = steps.len();
            if  len % 30 == 29 { loop_check(&steps.last().unwrap().matched, &node.limits)?; }
            match steps[len - 1].step()? {
                Some(s) => {
                    trace_pushing::<AndStep>(&s, steps.len());
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Ok(trace_end_walk(Path::And(steps)))
    }

    /// try to take a single step matching an And node
    fn step(&mut self) -> Result<Option<AndStep<'a>>, Error> {
        let mut step = AndStep {node: self.node,
                                matched: self.matched.next(0),
                                child_paths: Vec::<Path<'a>>::new(),
        };
        loop {
            let child_len = step.child_paths.len();
            if child_len == step.node.nodes.len() {
                break;    // all child nodes are satisfied, return success
            }
            let child_path = step.node.nodes[child_len].walk(step.matched.next(0))?;
            if child_path.limits().check(child_path.len()) == 0 {
                step.child_paths.push(child_path);
                // This could be done by removing the "else" below, but putting it here makes the trace up-to-date
                step.matched.set_end(step.child_paths.last().unwrap().end());
                trace_new_step(&step);
            } else if !step.back_off()? {
                return Ok(None);
            } else {
                step.matched.set_end(step.child_paths.last().unwrap().end());
            }
        }
        Ok(Some(step))
    }

    /// Back off a step after a failed match. It will back off repetitions until an untried one is found,
    /// leaving the **Node** in a state to proceed from there, or return **false** if the node cannot succeed
    ///
    /// It is important to keep in mind the difference between **XXXStep.back_off()** and **Path::back_off()**. For **Path**
    /// **back_off()** only touches the path - that is, the Vec(XXXStep). It can change Steps higher in the hierarchy
    /// or pop off the last step, but cannot change anything inside any of the *Step**s. **XXXStep::back_off()** ,
    /// on the other hand, should only change things within the current Step, that is 
    fn back_off(&mut self) -> Result<bool, Error> {
        if trace(6) {
            trace_indent(); println!("back off Node: {:?}", self);
            trace_change_indent(1);
        }
        let limits = self.node.limits;
        let mut ret = true;
        if limits.lazy() {
            println!("TODO");
            // TODO
            ret = false;
        }
        else {
            loop {
                // This pops off the last child path. If the Path backs off it is restored, if not then it is already removed
                if let Some(mut last_path) = self.child_paths.pop() {
                    if last_path.back_off()? {
                        self.child_paths.push(last_path);
                        break;
                    }
                } else { ret = false; break; }
                // backed off until reps are too few, discard
                if limits.check(self.child_paths.len()) != 0 { 
                    ret = false;
                    break;
                }
            };
        }
        if ret {
            self.matched.set_end(self.child_paths.last().unwrap().end());
        }
        if trace(6) {
            trace_change_indent(-1);
            trace_indent(); println!("back off Node: {}: {:?}", ret, self);
        }
        Ok(ret)
    }

    pub fn dump(&self, rank: usize, indent: usize) {
        trace_indent(); println!("|{0:1$}{2}: {3:?}", "", 4*indent, rank, self, );
        self.child_paths.iter().for_each(|x| x.dump(indent + 1));
    }
        
}

impl<'a> Walker<'a> for OrStep<'a> {
    /// Compiles a **Report** object from this path and its child after a successful search
    fn make_report(&'a self) -> Report {
        let subreports = self.child_path.gather_reports();
        Report { matched: self.matched,
                 name: self.node.named.clone(),
                 subreports}
    }
    fn name_details(&self) -> (&Option<String>, bool) { (&self.node.named, self.node.name_outside) }
    fn get_matched(&self) -> Matched { self.matched }
}

/// OR does not have a *step()* function because it cannot have a repeat count (to repeat an OR it must be enclosed in an AND)
impl<'a> OrStep<'a> {
    
    /// start a Path using an And node, matching as many times as it can subject to the matching algorithm (greedy or lazy)
    pub fn walk(node: &'a OrNode, matched: Matched) -> Result<Path<'a>, Error> {
        let mut steps = vec![OrStep {node, matched, child_path: Box::new(Path::None), which: 0}];
        trace_start_walk(&steps);
        for _i in 1..=node.limits.initial_walk_limit() {
            if let Some(s) = steps.last().unwrap().step()? {
                let len = steps.len();
                if  len % 30 == 29 { loop_check(&s.matched, &node.limits)?; }
                trace_pushing::<OrStep>(&s, steps.len());
                steps.push(s);
            } else { break; }
        }
        Ok(trace_end_walk(Path::Or(steps)))
    }
    
    /// try to take a single step matching an Or node
    fn step(&self) -> Result<Option<OrStep<'a>>, Error> {
        let mut step = OrStep {node: self.node,
                               matched: self.matched.next(0),
                               which: 0, 
                               child_path: Box::new(Path::None),
        };
        loop {
            if step.which == step.node.nodes.len() {
                if trace(4) { trace_indent(); println!("OR step failed (exhausted)"); }
                return Ok(None);
            }
            step.child_path = Box::new(step.node.nodes[step.which].walk(self.matched.next(0))?);
            if step.child_path.limits().check(step.child_path.len()) == 0 { break; }
            step.which += 1;
        }
        if trace(6) { trace_indent(); println!("    new OR step: {:?}", step); }
        step.matched.set_end(step.child_path.end());
        Ok(Some(step))
    }

    fn back_off(&mut self) -> Result<bool, Error> {
        if trace(6) {
            trace_indent(); println!("back off Node: {:?}", self);
            trace_change_indent(1);
        }
        let ret;
        loop {
            if self.child_path.back_off()? {
                ret = "true: child backed off";
                break;
            }
            if trace(6) { trace_indent(); println!("back off (next option){:?}", self); }
            self.which += 1;
            self.matched.set_end(self.matched.start);
            if self.which >= self.node.nodes.len() {
                ret = "false: exhausted";
                break;
            }
            self.child_path = Box::new(self.node.nodes[self.which].walk(self.matched)?);
            if self.child_path.limits().check(self.child_path.len()) == 0 {
                ret = "true: next option"; 
                break;
            }
        }
        if trace(6) { trace_change_indent(-1); trace_indent(); println!("back off Node: {}: {:?}", ret, self); }
        if !self.child_path.is_empty() {
            self.matched.set_end(self.child_path.end());
        }
        Ok(ret.starts_with("true"))
    }

    pub fn dump(&self, rank: usize, indent: usize) {
        trace_indent(); println!("|{0:1$}{2}: {3:?} {4} of {5}", "", 4*indent, rank, self, self.which, self.node.nodes.len());
        self.child_path.dump(indent + 1);
    }
}

/// **Matched** is used to keep track of the state of the search
/// string. It holds the whole string as well as offset to the
/// beginning and end of the substring matched by its owning **Step**.
#[derive(Copy,Clone)]
pub struct Matched {
//    /// the String where this match starts
//    pub full_string: &'a str,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    pub start: usize,
    /// The length of the string in bytes. Important: this is not in chars. Since it is in bytes the actual matching string is string[0..__match_len__]
    pub end: usize,
    /// the start of the string in characters
    pub char_start: usize,
}

impl Debug for Matched {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Input::apply_mut(|input| write!(f, "match \"{}\" [{}-{})", &input.full_text[self.start..self.end], self.start, self.end, ))
    }
}

impl Matched {
    /// Returns the length of the match in bytes
    pub fn len_bytes(&self) -> usize { self.end - self.start }

    /// Returns the length of the match in chars
    pub fn len_chars(&self) -> usize { Input::apply(|input| input.full_text[self.start..self.end].chars().count()) }

    /// Builds a new Matched object immediately following the one pointed to by self
    fn next(&self, len: usize) -> Matched {
        Matched {start: self.end, end: self.end + len, char_start: self.char_start + self.len_chars() }
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
        Input::apply(|input| {
            let s = &input.full_text[self.start..].chars().take(crate::get_abbrev()).collect::<String>();
            let dots = if s.len() < input.full_text.len() - self.start {"..."} else {""};
            format!("\"{}\"{}", s, dots)
        })
    }
}

/// **Source** holds information on where the input text comes from
#[derive(Default)]
enum Source {
    /// No input source has been set
    #[default] None,
    /// text is passed in as a string
    CmdLine,
    /// text should be read from STDIN
    Stdin(BufReader<std::io::Stdin>),
    /// files are read sequentlaiiy to get the input text
    File(BufReader<std::fs::File>)
}

impl Source {
    /// Extends by unit of BLOCK_SIZE if possible. Returns String to add along with boolean telling if the input is exhausted, or program error format
    fn extend(&mut self) -> Result<(String, bool), Error> {
        let mut string = "".to_string();
        let mut more  = true;
        match self {
            Source::CmdLine => more = false,
            Source::File(stream) => {
                while more && string.len() < Input::BLOCK_SIZE {
                    match stream.read_line(&mut string) {
                        std::io::Result::Err(error) => { return Err(Error::make(210, &error.to_string())); },
                        std::io::Result::Ok(0) => { more = false; break; },
                        std::io::Result::Ok(_bytes) => (),
                    }
                };
            },
            Source::Stdin(stream) => {
                while more && string.len() < Input::BLOCK_SIZE {
                    match stream.read_line(&mut string) {
                        std::io::Result::Err(error) => { return Err(Error::make(210, &error.to_string())); },
                        std::io::Result::Ok(bytes) => more = bytes > 0,
                    }
                };
            },
            Source::None => panic!("No input source has been set")
        }
        Ok((string, more))
    }
}

/// **Input** has a single static instance that holds the input text, and extends it as necessary by reading from stdin or files.
/// Using a static instance is not ideal, but seems to be the best solution. The program as originally written only took text
/// in the command and did not allow extending it from other sources. If rewriting from scratch perhaps having a common state
/// variable passed around might make more sense, though I did look at that and found that it was not simple since it had to
/// be mutable almost everywhere so it could be extended when needed, which severely limited the way it could be used. By
/// making it static the current input string is available everywhere to reference and can be updated as needed.
#[derive(Default)]
pub struct Input {
    /// The text currently in the buffer
    pub full_text: String,
    /// The source for getting more text
    source: Source,
    /// flag cleared when the input source is exhausted
    more_input: bool,
    filenames: Option<Vec<String>>,
    /// the current file in the file list being read, 0 if input is not from file
    fileno: usize
}

/// Single static value holding input text to search. All access to this shoulld use Input::apply() or Input::apply_mut()
static INPUT: Lazy<Mutex<Input>> = Lazy::new(|| Mutex::new(Input::default()));

impl Input {
    /// An indication of the block size to read in for extending input. The number is not exact since
    /// input is read line-by-line, but it is guaranteed that each extend() call adds at least this many bytes
    /// if they are available
    const BLOCK_SIZE: usize = 500;

    /// Initializes the static input. If TEXT is given that is searched directly, if empty FILES are accessed in
    /// sequential order to search, and if no files are given then STDIN is used
    pub fn init(text: String, files: Vec<String>) -> Result<(), Error> {
        let mut input = INPUT.lock().unwrap();
        if !text.is_empty() { input.init_cmdline(text)?; }
        else if !files.is_empty() {
            input.init_file(files[0].as_str())?;
            input.filenames = Some(files);
        } else { input.init_stdin()?; }
        Ok(())
    }

    /// Applies a mathod to the Input static instance. The instance cannot pass back pointers to any of its
    /// contents, this way lets it be called without needing to allocate memory to pass return values
    pub fn apply<T>(f: impl Fn(&Input) -> T) -> T {
        let input = &INPUT.lock().unwrap();
        f(input)
    }

    /// Like Input::apply() but allows functions with muts
    pub fn apply_mut<T>(mut f: impl FnMut(&Input) -> T) -> T {
        let input = &INPUT.lock().unwrap();
        f(input)
    }

    /// Moves to the next input source, a no-op for Cmdline and Stdin input. Returns TRUE if there is another input source, else FALSE
    pub fn next() -> Result<bool, Error> {
        let mut input = INPUT.lock().unwrap();
        let fileno = input.fileno + 1;
        let files_len = if let Some(filenames) = &input.filenames { filenames.len() } else { 0 };
        if fileno < files_len {
            let file = if let Some(filenames) = &input.filenames { filenames[fileno].clone()} else {String::new()};
            input.init_file(file.as_str())?; //filenames[fileno].as_str())?;
            input.fileno += 1;
            return Ok(true);
        }
        Ok(false)
    }

    /// Returns the sequence number of the file currently supplying input
    pub fn file_count() -> usize { Input::apply(|input| input.fileno) }
    
    //
    // Creation 
    //
    /// creates a text buffer getting the string from the command line
    fn init_cmdline(&mut self, text: String) -> Result<(), Error> {
        self.source = Source::CmdLine;
        self.full_text = text;
        Ok(())
    }

    /// creates a text buffer getting the string from stdin
    fn init_stdin(&mut self) -> Result<(), Error> {
        self.source = Source::Stdin(BufReader::new(std::io::stdin()));
        self.more_input = true;
        self._extend(1)?;  // any positive number forces a read
        Ok(())
    }

    /// creates a text buffer getting the string from the given file
    fn init_file(&mut self, filename: &str) -> Result<(), Error> {
        if trace(1) { println!("trying to open file {} for input", filename); }
        if filename == "-" { self.init_stdin() }
        else {
            match std::fs::File::open(filename) {
                Err(err) => {
                    let msg = format!("Error opening file {}: {}", filename, err);
                    Err(Error::make(201, &msg))
                },
                Ok(file) => {
                    self.source = Source::File(BufReader::new(file));
                    self.more_input = true;
                    self._extend(1)?;  // any positive number forces a read
                    Ok(())
                }
            }
        }
    }

    /// Public interface to _extend() method, tries to read into string to search so its length is greater than or equal to SIZE_BYTES
    pub fn extend(size_bytes: usize) -> Result<(), Error> {
        INPUT.lock().unwrap()._extend(size_bytes)
    }

    /// Checks that the input string is either fully read in or exceeds SIZE_BYTES in length
    fn _extend(&mut self, size_bytes: usize) -> Result<(), Error> {
        if self.more_input && self.full_text.len() < size_bytes {
            let (string, more) = self.source.extend()?;
            self.more_input = more;
            self.full_text.push_str(&string);
        }
        Ok(())
    }
    
    /// Like Input::extend() except prints any error and continues with the current string
    pub fn extend_quiet(size_bytes: usize) {
        if let Err(err) = Input::extend(size_bytes) {
            println!("Input error: {:?}", err);
        }
    }

    /// Returns the length of the current search text. It may be there is more text that still needs to be read in.
    pub fn len() -> usize { Input::apply(|input| input.full_text.len()) }

    /// For debugging, returns a String of the substring beginning at byte position FROM consisting of NUM_CHARS characters
    pub fn abbrev(from: usize, num_chars: usize) -> String {
        let input = INPUT.lock().unwrap();
        let mut chars: String = input.full_text[from..].chars().take(num_chars).collect();
        if from + chars.len() < input.full_text.len() { chars.push_str("..."); }
        chars
    }

    pub fn current_file(&self) -> Option<&str> {
        if let Some(filenames) = &self.filenames { Some(filenames[self.fileno].as_str()) }
        else { None }
    }
}

