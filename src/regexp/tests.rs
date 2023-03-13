//use super::*;
use crate::regexp::*;

fn make_chars_norep(string: &'static str) -> Node {
    Node::Chars(CharsNode{string: string.to_string(), limit_desc: (1, 1, false)})
}
fn make_chars_rep(string: &'static str, min: usize, max: usize, lazy: bool) -> Node {
    Node::Chars(CharsNode{string: string.to_string(), limit_desc: (min, max, lazy)})
}
fn make_special(special: char, min: usize, max: usize, lazy: bool) -> Node {
    Node::SpecialChar(SpecialCharNode {special, limit_desc: (min, max, lazy)})
}

fn make_and(min: usize, max: usize, lazy: bool) -> Node {
    Node::And(AndNode{nodes: Vec::<Node>::new(), limit_desc: (min, max, lazy)})
}
fn make_or() -> Node {
    Node::Or(OrNode{nodes: Vec::<Node>::new(), })
}

fn make_range(not: bool, min: usize, max: usize, lazy: bool) -> Node {
    Node::Range(RangeNode{not, targets: Vec::<Range>::new(), limit_desc: (min, max, lazy)})
}

impl Node {
    fn push(&mut self, node: Node) {
        match self {
            Node::And(and_node) => and_node.push(node),
            Node::Or(or_node) => or_node.push(node),
            _ => panic!("can only push to And or Or node")
        }
    }
}

#[test]
fn test_string_simple() {
    let mut node = make_and(1, 1, false);
    node.push(make_chars_norep("abcd"));
    node.push(Node::Success);
    assert_eq!(node, parse_tree("abcd").unwrap());
}

#[test]
fn test_string_embedded_reps() {
    let mut node = make_and(1, 1, false);
    node.push(make_chars_norep("ab"));
    node.push(make_chars_rep("c", 0, 1, false));
    node.push(make_chars_norep("de", ));
    node.push(make_chars_rep("f", 1, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_norep("gh", ));
    node.push(make_chars_rep("i", 0, EFFECTIVELY_INFINITE, false));
    node.push(Node::Success);
    assert_eq!(node, parse_tree("abc?def+ghi*").unwrap());
}
              
#[test]
fn test_string_embedded_reps_lazy() {
    let mut node = make_and(1, 1, false);
    node.push(make_chars_norep("ab"));
    node.push(make_chars_rep("c", 0, 1, true));
    node.push(make_chars_norep("de", ));
    node.push(make_chars_rep("f", 1, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_norep("gh", ));
    node.push(make_chars_rep("i", 0, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_norep("jk", ));
    node.push(Node::Success);
    assert_eq!(node, parse_tree("abc??def+?ghi*?jk").unwrap());
}
              
#[test]
fn test_special_in_string() {
    let mut node = make_and(1, 1, false);
    node.push(make_chars_norep("abc"));
    node.push(make_special('.', 1, 1, false));
    node.push(make_chars_norep("def", ));
    node.push(make_special('N', 0, 1, false));
    node.push(make_chars_norep("gh", ));
    node.push(make_special('.', 1, 3, false));
    node.push(make_chars_norep("ij", ));
    node.push(Node::Success);
    println!("{:#?}", parse_tree(r"abc.def\N?gh").unwrap());
    assert_eq!(node, parse_tree(r"abc.def\N?gh.{1,3}ij").unwrap());
}
    
