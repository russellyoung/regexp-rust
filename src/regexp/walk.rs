
use super::*;

// Used for debugging: the function trace(Path) either is a no-op or prints the given path, depending on the command line args
//static mut trace = |x| { println!("{:#?}", x) };
static mut PRINT_TRACE: bool = false;
pub fn set_walk_trace(turn_on: bool) { unsafe { PRINT_TRACE = turn_on }}

fn trace(path: Path) -> Path {
    unsafe { if PRINT_TRACE { println!("{}", path.desc(0)); }}
    path
}

// top level function 

// simplifies code a little
fn ref_last<T>(v: &Vec<T>) -> &T { &v[v.len() - 1] }

// used to hold the state at each step in the search walk, and later to build the final result if successful
#[derive(Debug)]
pub struct CharsStep<'a> {
    node: &'a CharsNode,
    string: &'a str,
    match_len: usize,
}

#[derive(Debug)]
pub struct SpecialStep<'a> {
    node: &'a SpecialCharNode,
    string: &'a str,
    match_len: usize,
}

#[derive(Debug)]
pub struct MatchingStep<'a> {
    node: &'a MatchingNode,
    string: &'a str,
    match_len: usize,
}

#[derive(Debug)]
pub struct AndStep<'a> {
    node: &'a AndNode,
    string: &'a str,
    match_len: usize,
    child_paths: Vec<Path<'a>>,
}

// Conceptually a Path takes as many steps as it can along the target string. A Path is a series of Steps along a particular branch, from 0 up to
// the maximum number allowed, or the maximum number matched, whichever is smaller. If continuing the search from the end of the Path fails it
// backtracks a Step and tries again.
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>), Special(Vec<SpecialStep<'a>>), Matching(Vec<MatchingStep<'a>>), And(Vec<AndStep<'a>>) }

use core::fmt::Debug;
impl<'a> Debug for Path<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.desc(0))
    }
}
impl<'a> Path<'a> {
    pub fn desc(&self, indent: usize) -> String {
        match self {
            Path::Chars(steps) => { println!("{:?}", steps); format!("{}chars({}): \"{}\"", pad(indent), steps.len() - 1, self.matched_string())},
            Path::Special(steps) => format!("{}special({}): \"{}\"", pad(indent), steps.len() - 1, self.matched_string()),
            Path::Matching(steps) => format!("{}matching({}): \"{}\"", pad(indent), steps.len() - 1, self.matched_string()),
            Path::And(steps) => {
                let mut msg = format!("{}and({}): \"{}\"", pad(indent), steps.len() - 1, self.matched_string());
                let last_step = ref_last(steps);
                for substep in last_step.child_paths.iter() {
                    msg.push_str(format!("\n{}", substep.desc(indent + 1)).as_str());
                }
                msg
            },
            _=> panic!("TODO")
        }
    }
    pub fn len(&self) -> usize {
        match self {
            Path::Chars(steps) => steps.len(),
            Path::Special(steps) => steps.len(),
            Path::Matching(steps) => steps.len(),
            Path::And(steps) => steps.len(),
//            _ => 0,
        }
    }
    fn match_len(&self) -> usize {
        match self {
            Path::Chars(steps) =>    steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::Special(steps) =>  steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::Matching(steps) => steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            Path::And(steps) =>      steps[0].string.len() - ref_last(steps).string.len() + ref_last(steps).match_len ,
            //            _ => &"",
        }
    }
    
    // gets the remaining target string at current path position
    fn matched_string(&self) -> &'a str {
        let match_len = self.match_len();
        match self {
            Path::Chars(steps) =>    { println!("matched_string: {:#?}", steps); &steps[0].string[0..match_len]},
            Path::Special(steps) =>  &steps[0].string[0..match_len],
            Path::Matching(steps) => &steps[0].string[0..match_len],
            Path::And(steps) =>      &steps[0].string[0..match_len],
            //            _ => &"",
        }
    }
    fn string_end(&self) -> &'a str {
        let len = self.match_len();
        match self {
            Path::Chars(steps) =>    &steps[0].string[len..],
            Path::Special(steps) =>  &steps[0].string[len..],
            Path::Matching(steps) => &steps[0].string[len..],
            Path::And(steps) =>      &steps[0].string[len..],
            //            _ => &"",
        }
    }
    fn in_limits(&self) -> bool {
        match self {
            Path::Chars(steps) => steps[0].node.limit_desc.0 < self.len() && self.len() <= steps[0].node.limit_desc.1,
            Path::Special(steps) => steps[0].node.limit_desc.0 < self.len() && self.len() <= steps[0].node.limit_desc.1,
            Path::Matching(steps) => steps[0].node.limit_desc.0 < self.len() && self.len() <= steps[0].node.limit_desc.1,
            Path::And(steps) => steps[0].node.limit_desc.0 < self.len() && self.len() <= steps[0].node.limit_desc.1,
        }
    }
    fn pop(&mut self) -> bool {
        match self {
            Path::Chars(steps) =>    { steps.pop(); (steps.len() > 0) && (steps.len() > steps[0].node.limit_desc.0)},
            Path::Special(steps) =>  { steps.pop(); (steps.len() > 0) && (steps.len() > steps[0].node.limit_desc.0)},
            Path::Matching(steps) => { steps.pop(); (steps.len() > 0) && (steps.len() > steps[0].node.limit_desc.0)},
            Path::And(steps) =>      { steps.pop(); (steps.len() > 0) && (steps.len() > steps[0].node.limit_desc.0)},
            //            _ => &"",
        }
    }
}

// Any way to make walk() generic?

impl<'a> CharsStep<'a> {
    pub fn walk(node: &'a CharsNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<CharsStep>::new();
        steps.push(CharsStep {node, string, match_len: 0});
        for _i in 1..(node.limit_desc.1 + 1) {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        trace(Path::Chars(steps))
    }
    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    fn step(&self) -> Option<CharsStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            Some(CharsStep {node: self.node, string, match_len: self.node.string.len()})
        } else { None }
    }
}

impl<'a> SpecialStep<'a> {
    pub fn walk(node: &'a SpecialCharNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<SpecialStep>::new();
        steps.push(SpecialStep {node, string, match_len: 0});
        for _i in 1..(node.limit_desc.1 + 1) {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        trace(Path::Special(steps))
    }
    fn step (&self) -> Option<SpecialStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            Some(SpecialStep {node: self.node, string, match_len: char_bytes(string, 1)})
        } else { None }
    }
}

impl<'a> MatchingStep<'a> {
    pub fn walk(node: &'a MatchingNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<MatchingStep>::new();
        steps.push(MatchingStep {node, string, match_len: 0});
        for _i in 1..(node.limit_desc.1 + 1) {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        trace(Path::Matching(steps))
    }
    fn step (&self) -> Option<MatchingStep<'a>> {
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            Some(MatchingStep {node: self.node, string, match_len: char_bytes(string, 1)})
        } else { None }
    }
}

impl<'a> AndStep<'a> {
    pub fn walk(node: &'a AndNode, string: &'a str) -> Path<'a> {
        let mut steps = Vec::<AndStep>::new();
        steps.push(AndStep {node, string, match_len: 0, child_paths: Vec::<Path<'a>>::new()});
        for _i in 1..(node.limit_desc.1 + 1) {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        trace(Path::And(steps))
    }
//    fn last_path(&self) -> &Path { &self.child_paths[self.child_paths.len() - 1] }
    fn step (&self) -> Option<AndStep<'a>> {
        let mut step = AndStep {node: self.node,
                                string: &self.string[self.match_len..],
                                match_len: 0,
                                child_paths: Vec::<Path<'a>>::new(),
        };
        loop {
            let child_len = step.child_paths.len();
            if child_len == step.node.nodes.len() { break; }
            let next_child_path = step.node.nodes[child_len].walk(if child_len == 0 { step.string } else {ref_last(&step.child_paths).string_end()});
            step.child_paths.push(next_child_path);
            if !ref_last(&step.child_paths).in_limits() {
                while step.back_off() {
                    if step.child_paths.is_empty() { return None; }
                }
            }
        }
        step.match_len = ref_last(&step.child_paths).string_end().len() - step.string.len();
        Some(step)
    }

    fn back_off(&mut self) -> bool {
        loop {
            let last_subpath = self.child_paths.len() - 1;
            if self.child_paths[last_subpath].pop() { break; }
            self.child_paths.pop();
            if self.child_paths.is_empty() { break; }
        }
        !self.child_paths.is_empty()
    }
}
