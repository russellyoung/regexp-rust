
use super::*;
use crate::trace;

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
#[derive(Debug)]
pub enum Path<'a> { Chars(Vec<CharsStep<'a>>), Special(Vec<SpecialStep<'a>>), Matching(Vec<MatchingStep<'a>>), And(Vec<AndStep<'a>>) }

//use core::fmt::Debug;
//impl<'a> Debug for Path<'a> {
//    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//        write!(f, "{}", self.desc(0))
//    }
//}

pub struct Report<'a> {
    found: &'a str,
    subreports: Box<Vec<Report<'a>>>,
}

impl<'a> Report<'a> {
    pub fn display(&self, indent: usize) {
        println!("{}\"{}\"", pad(indent), self.found);
        self.subreports.iter().for_each(move |r| r.display(indent + 1));
    }
}

impl<'a> Path<'a> {
    pub fn report(&self) -> Option<Report> {
        match self {
            Path::And(steps) => {
                if steps[0].node.report {
                    Some(Report {found: self.matched_string(),
                                 subreports: Box::new(ref_last(steps).child_paths.iter()
                                                      .map(|p| p.report())
                                                      .filter(|p| p.is_some())
                                                      .map(|p| p.unwrap())
                                                      .collect())})
                } else { None }
            },
            _ => None
        }
    }
    
    pub fn desc(&self, indent: usize) -> String {
        match self {
            Path::Chars(steps) => format!("{}chars '{}'{}: ({}): '{}'",
                                          pad(indent),
                                          steps[0].node.string,
                                          steps[0].node.limits.to_string(),
                                          steps.len() - 1,
                                          self.matched_string()),
            Path::Special(steps) => format!("{}special '{}'{}: ({}): '{}'",
                                            pad(indent),
                                            steps[0].node.special,
                                            steps[0].node.limits.to_string(),
                                            steps.len() - 1,
                                            self.matched_string()),
            Path::Matching(steps) => format!("{}matching {}{}: ({}): '{}'", 
                                            pad(indent),
                                            steps[0].node.targets_string(),
                                            steps[0].node.limits.to_string(),
                                            steps.len() - 1,
                                            self.matched_string()),
            Path::And(steps) => {
                let mut msg = format!("{}and({})({}): ({}): '{}'",
                                      pad(indent),
                                      steps[0].node.nodes.len(),
                                      steps[0].node.limits.to_string(),
                                      steps.len() - 1,
                                      self.matched_string());
                let last_step = ref_last(steps);
                for substep in last_step.child_paths.iter() {
                    msg.push_str(format!("\n{}", substep.desc(indent + 1)).as_str());
                }
                msg
            },
//            _=> panic!("TODO")
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
    pub fn matched_string(&self) -> &'a str {
        let match_len = self.match_len();
        match self {
            Path::Chars(steps) =>    &steps[0].string[0..match_len],
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
    fn pop(&mut self) -> bool {
        let limits = self.limits();
        match self {
            Path::Chars(steps) =>    { steps.pop();},
            Path::Special(steps) =>  { steps.pop();},
            Path::Matching(steps) => { steps.pop();},
            Path::And(steps) =>      { steps.pop();},
            //            _ => &"",
        };
        limits.min < self.len() && self.len() <= limits.max + 1
    }
    
    fn limits(&self) -> &'a Limits {
        match self {
            Path::Chars(steps) =>    &steps[0].node.limits,
            Path::Special(steps) =>  &steps[0].node.limits,
            Path::Matching(steps) => &steps[0].node.limits,
            Path::And(steps) =>      &steps[0].node.limits,
            //            _ => &"",
        }
    }
    fn trace(self) -> Path<'a> {
        if trace(0) { println!("pushing {}", self.desc(0)); }
        self
    }
}

// Any way to make walk() generic?

impl<'a> CharsStep<'a> {
    pub fn walk(node: &'a CharsNode, string: &'a str) -> Path<'a> {
        if trace(2) { println!(" -> WALK CHAR \"{}\" {}", node.string, abbrev(string)); }
        let mut steps = Vec::<CharsStep>::new();
        steps.push(CharsStep {node, string, match_len: 0});
        for _i in 1..=node.limits.max {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => break,
            }
        }
        if trace(2) { println!(" <- WALK CHAR \"{}\", {} reps", node.string, steps.len() - 1); }
        Path::Chars(steps).trace()
    }
    // this 'a -------------------------V caused me real problems, and needed help from Stackoverflow to sort out
    fn step(&self) -> Option<CharsStep<'a>> {
        if trace(4) {println!("    -> STEP CHAR \"{}\" ({})", self.node.string, abbrev(self.string));}
        let string = &self.string[self.match_len..];
        if self.node.matches(string) {
            if trace(4) {println!("    <- STEP CHAR \"{}\" matches \"{}\"", self.node.string, self.node.string);}
            Some(CharsStep {node: self.node, string, match_len: self.node.string.len()})
        } else {
            if trace(4) {println!("    <- STEP CHAR \"{}\": no match", self.node.string);}
            None
        }
    }
}

impl<'a> SpecialStep<'a> {
    pub fn walk(node: &'a SpecialCharNode, string: &'a str) -> Path<'a> {
        if trace(2) { println!(" -> WALK SPECIAL \"{}\" {}", node.special, abbrev(string)); }
        let mut steps = Vec::<SpecialStep>::new();
        steps.push(SpecialStep {node, string, match_len: 0});
        for _i in 1..=node.limits.max {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        if trace(2) { println!(" <- WALK SPECIAL \"{}\", {} reps", node.special, steps.len() - 1); }
        Path::Special(steps).trace()
    }
    fn step (&self) -> Option<SpecialStep<'a>> {
        let string = &self.string[self.match_len..];
        if trace(4) {println!("    -> STEP SPECIAL \"{}\" ({})", self.node.special, abbrev(string));}
        if self.node.matches(string) {
            let step = SpecialStep {node: self.node, string, match_len: char_bytes(string, char_bytes(string, 1))};
            if trace(4) {println!("    <- STEP SPECIAL \"{}\" matches \"{}\"", self.node.special, &string[0..step.match_len]);}
            Some(step)
        } else {
            if trace(4) {println!("    <- STEP SPECIAL \"{}\": no match", self.node.special);}
            None
        }
    }
}

impl<'a> MatchingStep<'a> {
    pub fn walk(node: &'a MatchingNode, string: &'a str) -> Path<'a> {
        if trace(2) { println!(" -> WALK MATCHING \"{}\" {}", node.targets_string(), abbrev(string)); }
        let mut steps = Vec::<MatchingStep>::new();
        steps.push(MatchingStep {node, string, match_len: 0});
        for _i in 1..=node.limits.max {
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        if trace(2) { println!(" <- WALK MATCHING \"{}\", {} reps", node.targets_string(), steps.len() - 1); }
        Path::Matching(steps).trace()
    }
    fn step (&self) -> Option<MatchingStep<'a>> {
        let string = &self.string[self.match_len..];
        if trace(4) {println!("    -> STEP MATCHING {} ({})", self.node.targets_string(), abbrev(string));}
        if self.node.matches(string) {
            let step = MatchingStep {node: self.node, string, match_len: char_bytes(string, 1)};
            if trace(4) {println!("    <- STEP MATCHING {} matches \"{}\"", self.node.targets_string(), &string[0..step.match_len]);}
            Some(step)
        } else {
            if trace(4) {println!("    <- STEP SPECIAL \"{}\": no match", self.node.targets_string());}
            None
        }
    }
}

impl<'a> AndStep<'a> {
    pub fn walk(node: &'a AndNode, string: &'a str) -> Path<'a> {
        if trace(2) { println!(" -> WALK AND({}) {}", node.nodes.len(), abbrev(string)); }
        let mut steps = Vec::<AndStep>::new();
        steps.push(AndStep {node, string, match_len: 0, child_paths: Vec::<Path<'a>>::new()});
        for i in 1..=node.limits.max {
            if trace(6) {println!("    WALK AND loop {} of {}", i, node.nodes.len());}
            match ref_last(&steps).step() {
                Some(s) => steps.push(s),
                None => { break; }
            }
        }
        if trace(2) { println!(" <- WALK AND({}), {} reps", node.nodes.len(), steps.len() - 1); }
        Path::And(steps).trace()
    }

    fn status(&self) -> String {
        let mut status = format!("    AND({}) state: [", self.node.nodes.len());
        for p in self.child_paths.iter() { status.push_str(&format!("{}, ", p.len())); }
        for _i in self.child_paths.len()..self.node.nodes.len() { status.push_str(&"-, ".to_string()); }
        status.push_str(&"]".to_string());
        status
    }

    //    fn last_path(&self) -> &Path { &self.child_paths[self.child_paths.len() - 1] }
    fn step (&self) -> Option<AndStep<'a>> {
        let string0 = &self.string[self.match_len..];
        if trace(4) {println!("    -> STEP AND {} ({})", self.node.nodes.len(), abbrev(string0));}
        let mut step = AndStep {node: self.node,
                                string: string0,
                                match_len: 0,
                                child_paths: Vec::<Path<'a>>::new(),
        };
        loop {
            let child_len = step.child_paths.len();
            if child_len == step.node.nodes.len() { break; }
            let string = if child_len == 0 { step.string } else {ref_last(&step.child_paths).string_end()};
            if trace(6) { println!("    AND SUBSTEP({}) : {} : limits[{}, {}] : rep {} : {}",
                                   step.node.nodes.len(),
                                   step.status(),
                                   step.node.limits.min,
                                   step.node.limits.max,
                                   child_len,
                                   abbrev(string)); };
            let child_path = step.node.nodes[child_len].walk(string);
            let child_limits = child_path.limits();
            if child_limits.min + 1 <= child_path.len() {
                if trace(3) {println!("push child_path {:#?}", child_path.desc(0));}
                step.child_paths.push(child_path);
            } else if !step.back_off() {
                if trace(4) {println!("    <- STEP AND ({}): no match", self.node.nodes.len());}
                return None;
            }
        }
        step.match_len = step.string.len() - ref_last(&step.child_paths).string_end().len();
        if trace(4) {println!("    <- STEP AND {} matches \"{}\"", self.node.nodes.len(), &step.string[0..step.match_len]);}
        Some(step)
    }
    
    fn back_off(&mut self) -> bool {
        if trace(2) { println!("backing off"); }
        if self.child_paths.is_empty() { return false; }
        loop {
            let last_pathnum = self.child_paths.len() - 1;
            if self.child_paths[last_pathnum].pop() { break; }
            self.child_paths.pop();
            if self.child_paths.len() == 0 { break; }
        }
        if trace(4) { println!("Leaving backoff with status {}", self.status()); }
        !self.child_paths.is_empty()
    }
}
    
const ABBREV_LEN: usize = 5;
fn abbrev(string: &str) -> String { if string.len() < ABBREV_LEN {format!("\"{}\"", string)} else {format!("\"{}...\"", &string[0..char_bytes(&string, ABBREV_LEN)])} }
