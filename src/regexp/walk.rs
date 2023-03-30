
use super::*;
use crate::{trace, trace_indent, trace_change_indent};

// simplifies code a little
fn ref_last<T>(v: &Vec<T>) -> &T { &v[v.len() - 1] }

//////////////////////////////////////////////////////////////////
//
// Report
//
// Used to deliver the results to the caller, a tree of results
//
//////////////////////////////////////////////////////////////////
pub struct Report<'a> {
    pub found: &'a str,
    pub subreports: Vec<Report<'a>>,
}

impl<'a> Report<'a> {
    pub fn display(&self, indent: usize) {
        println!("{}\"{}\"", pad(indent), self.found);
        self.subreports.iter().for_each(move |r| r.display(indent + 1));
    }
}

//////////////////////////////////////////////////////////////////
//
// Step structs
//
// A path through the tree is a vector of steps, where each step is a
// single match for its node.  The steps are used to walk through the
// trtee and to build the Report structure for the caller.
//
//////////////////////////////////////////////////////////////////
pub struct CharsStep<'a> {
    node: &'a CharsNode,
    string: &'a str,
    // Important: match_len is in bytes, not characters
    match_len: usize,
}

pub struct SpecialStep<'a> {
    node: &'a SpecialCharNode,
    string: &'a str,
    match_len: usize,
}

pub struct SetStep<'a> {
    node: &'a SetNode,
    string: &'a str,
    match_len: usize,
}

pub struct AndStep<'a> {
    node: &'a AndNode,
    string: &'a str,
    match_len: usize,
    child_paths: Vec<Path<'a>>,
}

pub struct OrStep<'a> {
    node: &'a OrNode,
    string: &'a str,
    match_len: usize,
    child_path: Box<Path<'a>>,
    which: usize,
}

//////////////////////////////////////////////////////////////////
//
// Path struct
//
// Conceptually a Path takes as many steps as it can along the target
// string. A Path is a series of Steps along a particular branch, from
// 0 up to the maximum number allowed, or the maximum number matched,
// whichever is smaller. If continuing the search from the end of the
// Path fails it backtracks a Step and tries again.
//
//////////////////////////////////////////////////////////////////
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>), Special(Vec<SpecialStep<'a>>), Set(Vec<SetStep<'a>>), And(Vec<AndStep<'a>>), Or(OrStep<'a>), None }

impl<'a> Path<'a> {
    pub fn report(&self) -> Option<Report> {
        match self {
            Path::And(steps) => {
                if steps[0].node.report {
                    Some(Report {found: self.matched_string(),
                                 subreports: ref_last(steps).child_paths.iter()
                                 .filter_map(|p| p.report())
                                 .collect()})
                } else { None }
            },
            Path::Or(step) => step.child_path.report(),
            _ => None
        }
    }
    
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
    
    // gets the remaining target string at current path position
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
        if trace(1) { println!("{}backoff: {:?}, success: {}", trace_indent(), self, ret)}
        ret
    }

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

    fn trace(self, level: u32, prefix: &'a str) -> Path {
        if trace(level) {
            trace_change_indent(-2);
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
        write!(f, "AND({}){} state [{}], string {}", self.node.nodes.len(), self.node.limits().simple_display(), child_counts, abbrev(self.string))
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
    pub fn walk(node: &'a CharsNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<CharsStep>::new();
        steps.push(CharsStep {node, string, match_len: 0});
        if trace(1) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(2);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(2) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => break,
            }
        }
        Path::Chars(steps).trace(1, "end walk")
    }
    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    fn step(&self) -> Option<CharsStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            Some(CharsStep {node: self.node, string, match_len: self.node.string.len()})
        } else {
            None
        }
    }
}

impl<'a> SpecialStep<'a> {
    pub fn walk(node: &'a SpecialCharNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<SpecialStep>::new();
        steps.push(SpecialStep {node, string, match_len: 0});
        if trace(1) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(2);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(2) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::Special(steps).trace(1, "end walk")
    }
    fn step (&self) -> Option<SpecialStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            let step = SpecialStep {node: self.node, string, match_len: char_bytes(string, 1)};
            Some(step)
        } else {
            None
        }
    }
}

impl<'a> SetStep<'a> {
    pub fn walk(node: &'a SetNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<SetStep>::new();
        steps.push(SetStep {node, string, match_len: 0});
        if trace(1) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(2);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(2) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::Set(steps).trace(1, "end walk")
    }
    fn step (&self) -> Option<SetStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            let step = SetStep {node: self.node, string, match_len: char_bytes(string, 1)};
            Some(step)
        } else {
            None
        }
    }
}

impl<'a> AndStep<'a> {
    pub fn walk(node: &'a AndNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<AndStep>::new();
        steps.push(AndStep {node, string, match_len: 0, child_paths: Vec::<Path<'a>>::new()});
        if trace(1) {
            println!("{}Starting walk for {:?}", trace_indent(), &steps[0]);
            trace_change_indent(2);
        }
        for _i in 1..=node.limits().initial_walk_limit() {
            match ref_last(&steps).step() {
                Some(s) => {
                    if trace(2) { println!("{}Pushing {:?} rep {}", trace_indent(), s, steps.len() + 1); }
                    steps.push(s);
                },
                None => { break; }
            }
        }
        Path::And(steps).trace(1, "end walk")
    }

/*
    fn status(&self) -> String {
        let mut status = format!("    AND({}) state: [", self.node.nodes.len());
        for p in self.child_paths.iter() { status.push_str(&format!("{}, ", p.len())); }
        for _i in self.child_paths.len()..self.node.nodes.len() { status.push_str("-, "); }
        status.push(format!("]{}", self.limits.simple_display()));
        status
    }
*/
    //    fn last_path(&self) -> &Path { &self.child_paths[self.child_paths.len() - 1] }
    fn step (&self) -> Option<AndStep<'a>> {
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
                    trace_change_indent(-2);
                    println!("{}new step: {:?}", trace_indent(), step);
                    trace_change_indent(2);
                }
            } else if !step.back_off() {
                return None;
            }
        }
        step.match_len = step.string.len() - ref_last(&step.child_paths).string_end().len();
        Some(step)
    }

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
}
                    
    // OR is a little different: no repeat count, so its step() is not needed
impl<'a> OrStep<'a> {
    pub fn walk(node: &'a OrNode, string: &'a str) -> Path<'a> {
        if trace(1) {
            let fake = OrStep {node, string, which: 0, child_path: Box::new(Path::None), match_len: 0};
            println!("{}Starting walk for {:?}", trace_indent(), fake);
            trace_change_indent(2);
        }
        for which in 0..node.nodes.len() {
            let child_path = node.nodes[which].walk(string);
            if child_path.limits().check(child_path.len()) == 0 {
                let match_len = child_path.match_len();
                return Path::Or(OrStep {node, string, which, child_path: Box::new(child_path), match_len}).trace(1, "end walk0");
            }
        }
        Path::Or(OrStep {node, string, which: node.nodes.len(), child_path: Box::new(Path::None), match_len: 0}).trace(1, "end walk1")
    }
//    fn status(&self) -> String {
//        format!("    OR({}) which: {} reps: {}", self.node.nodes.len(), self.which, self.child_path.len())
//    }

}

// helper function to keep strings from being too long.
pub fn set_abbrev_size(size: u32) { unsafe {ABBREV_LEN = size as usize; }}
static mut ABBREV_LEN: usize = 5;
fn abbrev(string: &str) -> String {
    let s:String = string.chars().take(unsafe {ABBREV_LEN}).collect();
    let dots = if s.len() == string.len() {""} else {"..."};
    format!("\"{}\"{}", s, dots)
}
