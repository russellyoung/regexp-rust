use crate::regexp::*;

//
// Initial tests are basic sanity tests for the tree parser. They are relatively simple because the
// search tests (TODO) will provide more complete testing. These are intended mainly as a sanity check
// make sure that the parsing basically works.
//

fn make_chars_string(string: &'static str) -> Node {
    Node::Chars(CharsNode{string: string.to_string(), limits: Limits{min: 1, max: 1, lazy: false}})
}
fn make_chars_single(string: &'static str, min: usize, max: usize, lazy: bool) -> Node {
    Node::Chars(CharsNode{string: string.to_string(), limits: Limits{min, max, lazy}})
}
fn make_special(special: char, min: usize, max: usize, lazy: bool) -> Node {
    Node::SpecialChar(SpecialCharNode {special, limits: Limits {min, max, lazy}})
}

fn make_and(min: usize, max: usize, lazy: bool, report: bool) -> Node {
    Node::And(AndNode{nodes: Vec::<Node>::new(), limits: Limits{min, max, lazy}, report})
}
fn make_or() -> Node {
    Node::Or(OrNode{nodes: Vec::<Node>::new(), })
}

fn make_set(not: bool, targets: Vec<Set>, min: usize, max: usize, lazy: bool) -> Node {
    Node::Set(SetNode{not, targets, limits: Limits{min, max, lazy}})
}
fn push_sets(set_node: &mut SetNode, sets: &mut Vec<Set>) {
    set_node.targets.append(sets);
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

//
// parse tests
//
#[test]
fn test_string_simple() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("abcd"));
    assert_eq!(node, parse_tree("abcd").unwrap());
}

#[test]
fn test_string_embedded_reps() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("ab"));
    node.push(make_chars_single("c", 0, 1, false));
    node.push(make_chars_string("de", ));
    node.push(make_chars_single("f", 1, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_string("gh", ));
    node.push(make_chars_single("i", 0, EFFECTIVELY_INFINITE, false));
    assert_eq!(node, parse_tree("abc?def+ghi*").unwrap());
}
              
#[test]
fn test_string_embedded_reps_lazy() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("ab"));
    node.push(make_chars_single("c", 0, 1, true));
    node.push(make_chars_string("de", ));
    node.push(make_chars_single("f", 1, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string("gh", ));
    node.push(make_chars_single("i", 0, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string("jk", ));
    assert_eq!(node, parse_tree("abc??def+?ghi*?jk").unwrap());
}
              
#[test]
fn test_special_in_string() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("abc"));
    node.push(make_special('.', 1, 1, false));
    node.push(make_chars_string("def", ));
    node.push(make_special('N', 0, 1, false));
    node.push(make_chars_string("gh", ));
    node.push(make_special('.', 1, 3, false));
    node.push(make_chars_string("ij", ));
    println!("{:#?}", parse_tree(r"abc.def\N?gh").unwrap());
    assert_eq!(node, parse_tree(r"abc.def\N?gh.{1,3}ij").unwrap());
}
    
#[test]
fn or_with_chars_bug() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("ab"));
    let mut or_node = make_or();
    or_node.push(make_chars_string("c"));
    or_node.push(make_chars_string("d"));
    node.push(or_node);
    node.push(make_chars_string("ef"));
    assert_eq!(node, parse_tree(r"abc\|def").unwrap());
}

#[test]
fn set_basic() {
    let mut node = make_and(1, 1, false, true);
    node.push(make_chars_string("ab"));
    let targets = vec![Set::RegularChars("cde".to_string()),];
    node.push(make_set(false, targets, 0, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_string("fg"));
    let targets = vec![Set::RegularChars("h".to_string()),
                       Set::Range('i', 'k'),
                       Set::RegularChars("lm".to_string()),
                       Set::SpecialChar('N')];
    node.push(make_set(true, targets, 1, 1, false));
    assert_eq!(node, parse_tree(r"ab[cde]*fg[^hi-klm\N]").unwrap());
}


fn find<'a>(re: &'a str, text: &'a str, expected: &'a str) {
    let tree = parse_tree(re).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let path = walk_tree(&tree, text).unwrap_or_else(|| panic!("Expected \"{}\", didn't find anything", expected));
    let report = path.report().unwrap_or_else(|| panic!("re \"{}\" expected \"{}\", got no match", re, expected));
    assert_eq!(report.found, expected, "re \"{}\" expected \"{}\", found \"{}\"", re, expected, report.found);
}       
        
fn not_find<'a>(re: &'a str, text: &'a str) {
    let tree = parse_tree(re).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    assert!(walk_tree(&tree, text).is_none(), "re \"{}\" expected no match, found one", re);
}       

//
// walk tests
//
#[test]
fn simple_chars() {
    find("abc", "abcd", "abc");
    find("bcd", "abcd", "bcd");
    find("bcd", "abcde", "bcd");
    not_find("bcd", "abde");
}
    
fn rep_chars() {
    find("abc*d", "abcdz", "abcd");
    find("abc+d", "abcdz", "abcd");
    find("abc*d", "abccdz", "abccd");
    find("abc*d", "abdz", "abd");
    not_find("abc+d", "abd");
    not_find("abc{2}d", "abcdz");
    find("abc{2}d", "abccdz", "abccd");
    not_find("abc{2}d", "abcccdz");
    find("abc{2,3}d", "abcccdz", "abcccd");
}
    
#[test]
fn unicode() {
    find("abc", "ab你好abcd", "abc");
    find("你好", "ab你好abcd", "你好");
    find("你好you-all", "ab你好you-allabcd", "你好you-all");
    find("a.*a", "qqab你好abcd", "ab你好a");
}

    
#[test]
fn special_chars() {
    find("ab.de", "aabcdef", "abcde");
    find("ab.*de", "abcdedefg", "abcdede");
    not_find("cde.", "abcde");
    find(".*", "ab1234fg", "ab1234fg");
    find(r"\N+", "ab1234fg", "1234");
    find(r"\N*", "ab1234fg", "");
    find(r"b\N*", "ab1234fg", "b1234");
}
    
#[test]
fn set_chars() {
    find(r"[abc]+", "xabacaacd", "abacaac");
    find(r"z[abc]*z", "abzzcd", "zz");
    find(r"z[abc]*z", "abzaabczcd", "zaabcz");
    not_find(r"z[abc]*z", "abzaabcdzcd");
    find(r"z[a-m]*z", "abzabclmzxx", "zabclmz");
    find(r"[a-mz]+", "xyzabclmzxx", "zabclmz");
}

#[test]
fn non_set_chars() {
    find("a[^hgf]*", "aabcdefghij", "aabcde");
    find("a[^e-m]*", "aabcdefghij", "aabcd");
    find("a[^-e-m]*", "xab-cdefghij", "ab");
    not_find("[^abcd]+", "abcdab");
}

#[test]
fn or() {
    find(r"abc\|de", "xxxabceyy", "abce");
    find(r"abc\|de", "xxxabdeyy", "abde");
    find(r"abc\|d", "xxxabdeyy", "abd");
    find(r"c\|de", "xxxabdeyy", "de");
    find(r"c\|d", "c", "c");
    find(r"c\|d", "d", "d");
    find(r"abc\|d*e", "xxxabceyy", "abce");
    not_find(r"abc\|d*e", "xxxabcceyy");
    find(r"abc\|d*e", "xxxabdeyy", "abde");
    find(r"abc\|d*e", "xxxabeyy", "abe");
    find(r"abc\|d*e", "xxxabdddeyy", "abddde");

    find(r"\(abc\)\|de", "xxxabcdeyy", "de");
    find(r"\(abc\)\|de", "xxxabceyy", "abce");
    find(r"x\(abc\)\|de", "xxxabceyy", "xabce");
    not_find(r"x\(abc\)\|de", "xxxabeyy");
    find(r"x\(abc\)+\|\(de\)*d", "xxxabcabcd", "xabcabcd");
    find(r"x\(abc\)+\|\(de\)*d", "xxxdedededx", "xdededed");
//    find(r"x\(abc\)+\|\(de\)*d", "xxxdedededee", "xdedededed");
    find(r"x\(abc\)+\|\(de\)*d", "xxxdededede", "xdededed");
}
