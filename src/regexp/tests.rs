use crate::regexp::*;
use std::io::Write;
//
// Initial tests are basic sanity tests for the tree parser. They are relatively simple because the
// search tests (TODO) will provide more complete testing. These are intended mainly as a sanity check
// make sure that the parsing basically works.
//

#[test]
fn peekable() {
    let mut chars = Peekable::new("abcde");
    chars.push('z');
    assert_eq!(Some('a'), chars.next());
    assert_eq!(Some('b'), chars.peek());
    assert_eq!(Some('b'), chars.peek());
    assert_eq!(Some('b'), chars.next());
    assert_eq!(Some('c'), chars.peek());
    chars.put_back('x');
    assert_eq!(Some('x'), chars.peek());
    assert_eq!(Some('x'), chars.next());
    assert_eq!(Some('c'), chars.next());
    assert_eq!((Some('d'), Some('e')), chars.peek_2());
    let peek_4 = chars.peek_n(4);
    assert!(peek_4.len() == 4
            && Some('d') == peek_4[0]
            && Some('e') == peek_4[1]
            && Some('z') == peek_4[2]
            && None == peek_4[3]);
    assert_eq!(Some('d'), chars.next());
    assert_eq!(Some('e'), chars.next());
    assert_eq!(Some('z'), chars.next());
    assert_eq!(None, chars.next());
    assert_eq!(None, chars.peek());
    
}

//
// test Limits: parse all modes, confirm check() works. Also kind of tests Peekable
//
#[test]
fn limits_test() {
    let limits_string = " ? * + {2} {3,5} {6,} ?? *? +? {2}? {3,5}? {6,}? ";
    let data: [(usize, usize, bool); 13] = [(1, 1, false),
                                            (0, 1, false),
                                            (0, EFFECTIVELY_INFINITE, false),
                                            (1, EFFECTIVELY_INFINITE, false),
                                            (2, 2, false),
                                            (3, 5, false),
                                            (6, EFFECTIVELY_INFINITE, false),
                                            (0, 1, true),
                                            (0, EFFECTIVELY_INFINITE, true),
                                            (1, EFFECTIVELY_INFINITE, true),
                                            (2, 2, true),
                                            (3, 5, true),
                                            (6, EFFECTIVELY_INFINITE, true),
    ];
    let mut chars = Peekable::new(limits_string); 
    for (min, max, lazy) in data {
        if let Ok(limits) = Limits::parse(&mut chars) {
            assert!(chars.next().unwrap() == ' ', "unexpected parse results");
            assert!(limits.check(min) < 0, "< min check failed for ({}, {}, {})", min, max, lazy);
            assert!(limits.check(min + 1) == 0, "= min check failed for ({}, {}, {})", min, max, lazy);
            assert!(limits.check(max + 1) == 0, "= max check failed for ({}, {}, {})", min, max, lazy);
            assert!(limits.check(max + 2) > 0, "> max check failed for ({}, {}, {})", min, max, lazy);
            assert!(limits.lazy == lazy, "lazy check failed for ({}, {}, {})", min, max, lazy);
        } else { panic!("failed parsing Limits"); }
    }
    assert!(chars.next() == None, "Failed to consume test string");
}

//
// parser tests
//
fn make_chars_string(blocks: Vec<CharsContents>) -> Node {
        Node::Chars(CharsNode{blocks, limits: Limits::default(), named: None})
}
fn make_chars_single(block: CharsContents, min: usize, max: usize, lazy: bool) -> Node {
    Node::Chars(CharsNode{blocks: vec![block], limits: Limits{min, max, lazy}, named: None})
}

fn make_root<'a> (min: usize, max: usize, lazy: bool) -> Node { make_and(min, max, lazy, Some(""))}

fn make_and<'a> (min: usize, max: usize, lazy: bool, name: Option<&'a str>) -> Node {
    let named = {if let Some(n) = name {Some(n.to_string())} else {None}};
    Node::And(AndNode{nodes: Vec::<Node>::new(), limits: Limits{min, max, lazy}, named, anchor: false})
}
fn make_or() -> Node {
    Node::Or(OrNode{nodes: Vec::<Node>::new(), limits: Limits::default(), named: None})
}

//fn make_set(not: bool, targets: Vec<SetUnit>, min: usize, max: usize, lazy: bool) -> Node {
//    Node::SetUnit(SetNode{not, targets, limits: Limits{min, max, lazy}, named: None})
//}
//fn push_sets(set_node: &mut SetNode, sets: &mut Vec<Set>) {
//    set_node.targets.append(sets);
//}

impl Node {
    fn push(&mut self, node: Node) {
        match self {
            Node::And(and_node) => and_node.nodes.push(node),
            Node::Or(or_node) => or_node.nodes.push(node),
            _ => panic!("can only push to And or Or node")
        }
    }
}

//
// parse tests
//
#[test]
fn test_string_simple() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string(vec![CharsContents::Regular("abcd".to_string())]));
    assert_eq!(node, parse_tree("abcd", false).unwrap());
}

#[test]
fn test_string_embedded_reps_greedy() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string(vec![CharsContents::Regular("ab".to_string())]));
    node.push(make_chars_single(CharsContents::Regular("c".to_string()), 0, 1, false));
    node.push(make_chars_string(vec![CharsContents::Regular("de".to_string())], ));
    node.push(make_chars_single(CharsContents::Regular("f".to_string()), 1, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_string(vec![CharsContents::Regular("gh".to_string())], ));
    node.push(make_chars_single(CharsContents::Regular("i".to_string()), 0, EFFECTIVELY_INFINITE, false));
    assert_eq!(node, parse_tree("abc?def+ghi*", false).unwrap());
}
              
#[test]
fn test_string_embedded_reps_lazy() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string(vec![CharsContents::Regular("ab".to_string())]));
    node.push(make_chars_single(CharsContents::Regular("c".to_string()), 0, 1, true));
    node.push(make_chars_string(vec![CharsContents::Regular("de".to_string())]));
    node.push(make_chars_single(CharsContents::Regular("f".to_string()), 1, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string(vec![CharsContents::Regular("gh".to_string())]));
    node.push(make_chars_single(CharsContents::Regular("i".to_string()), 0, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string(vec![CharsContents::Regular("jk".to_string())]));
    assert_eq!(node, parse_tree("abc??def+?ghi*?jk", false).unwrap());
}

#[test]
fn or_with_chars_bug() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string(vec![CharsContents::Regular("ab".to_string())]));
    let mut or_node = make_or();
    or_node.push(make_chars_string(vec![CharsContents::Regular("c".to_string())]));
    or_node.push(make_chars_string(vec![CharsContents::Regular("d".to_string())]));

    node.push(or_node);
    node.push(make_chars_string(vec![CharsContents::Regular("ef".to_string())]));
    assert_eq!(node, parse_tree(r"abc\|def", false).unwrap());
}

// #[test]
// fn set_basic() {
//     let mut node = make_root(1, 1, false);
//     node.push(make_chars_string(vec![CharsContents::Regular("ab".to_string())]));
//     let targets = vec![Set::RegularChars("cde".to_string()),];
//     node.push(make_set(false, targets, 0, EFFECTIVELY_INFINITE, false));
//     node.push(make_chars_string(vec![CharsContents::Regular("fg".to_string())]));
//     let targets = vec![Set::RegularChars("h".to_string()),
//                        Set::Range('i', 'k'),
//                        Set::RegularChars("lm".to_string()),
//                        Set::SpecialChar('d')];
//     node.push(make_set(true, targets, 1, 1, false));
//     assert_eq!(node, parse_tree(r"ab[cde]*fg[^hi-klm\d]", false).unwrap());
// }


fn find<'a>(alt: bool, re: &'a str, text: &'a str, expected: &'a str) {
    print!("RUNNING '{}' '{}'... ", re, text);
    std::io::stdout().flush().unwrap();
    let tree = parse_tree(re, alt).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let (path, _) = walk_tree(&tree, text)
        .unwrap_or_else(|err| panic!("Expected \"{}\", got error '{}'", expected, err))
        .unwrap_or_else(|| panic!("Expected {}, found none", expected));
    assert_eq!(path.matched_string(), expected, "re \"{}\" expected \"{}\", found \"{}\"", re, expected, path.matched_string());
    println!("OK");
}       
        
fn not_find<'a>(alt: bool, re: &'a str, text: &'a str) {
    let tree = parse_tree(re, alt).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    assert!(walk_tree(&tree, text).unwrap().is_none(), "re \"{}\" expected no match, found one", re);
}       

//
// walk tests
//
#[test]
fn simple_chars() {
    find(false, "abc", "abcd", "abc");
    find(false, "bcd", "abcd", "bcd");
    find(false, "bcd", "abcde", "bcd");
    not_find(false, "bcd", "abde");
}
    
fn rep_chars() {
    find(false, "abc*d", "abcdz", "abcd");
    find(false, "abc+d", "abcdz", "abcd");
    find(false, "abc*d", "abccdz", "abccd");
    find(false, "abc*d", "abdz", "abd");
    not_find(false, "abc+d", "abd");
    not_find(false, "abc{2}d", "abcdz");
    find(false, "abc{2}d", "abccdz", "abccd");
    not_find(false, "abc{2}d", "abcccdz");
    find(false, "abc{2,3}d", "abcccdz", "abcccd");
    find(false, r"a\d*\|b*c", "aacc", "ac");
    find(false, r"a\d*\|b*c", "aabbbcc", "abbbc");
    find(false, r"a\d*\|b*c", "a12c", "a12c");
    not_find(false, r"a\d*\|b*c", "aa1bcc");
}
    
#[test]
fn unicode() {
    find(false, "abc", "ab你好abcd", "abc");
    find(false, "你好", "ab你好abcd", "你好");
    find(false, "你好you-all", "ab你好you-allabcd", "你好you-all");
    find(false, "a.*a", "qqab你好abcd", "ab你好a");
    find(false, r"是很*好", "这是很很很好",  "是很很很好");
}

    
#[test]
fn special_chars() {
    find(false, "ab.de", "aabcdef", "abcde");
    find(false, "ab.*de", "abcdedefg", "abcdede");
    not_find(false, "cde.", "abcde");
    find(false, ".*", "ab1234fg", "ab1234fg");
    find(false, r"\d+", "ab1234fg", "1234");
    find(false, r"\d*", "ab1234fg", "");
    find(false, r"b\d*", "ab1234fg", "b1234");
    not_find(false, r"xxx$", "abcxxxy");
    find(false, r"xxx$", "abcxxx", "xxx");
    find(false, r"xxx\$z", "abcxxx$zx", "xxx$z");
    find(false, r"^abc", "abcdef", "abc");
    not_find(false, r"^abc", "xabcdef");
    find(false, r"a^bc", "xa^bcdef", "a^bc");
    find(false, r"a\d+\l+", "aba123Ba123bcD", "a123bc");
    find(false, r"\a\u+", "你好abCD没有", "bCD");
}
    
// #[test]
// fn set_chars() {
//     find(r"[abc]+", "xabacaacd", "abacaac");
//     find(r"z[abc]*z", "abzzcd", "zz");
//     find(r"z[abc]*z", "abzaabczcd", "zaabcz");
//     not_find(r"z[abc]*z", "abzaabcdzcd");
//     find(r"z[a-m]*z", "abzabclmzxx", "zabclmz");
//     find(r"[a-mz]+", "xyzabclmzxx", "zabclmz");
// }

// #[test]
// fn non_set_chars() {
//     find("a[^hgf]*", "aabcdefghij", "aabcde");
//     find("a[^e-m]*", "aabcdefghij", "aabcd");
//     find("a[^-e-m]*", "xab-cdefghij", "ab");
//     not_find("[^abcd]+", "abcdab");
// }

#[test]
fn basic_or() {
    find(false, r"abc\|de", "xxxabceyy", "abce");
    find(false, r"abc\|de", "xxxabdeyy", "abde");
    find(false, r"abc\|d", "xxxabdeyy", "abd");
    find(false, r"c\|de", "xxxabdeyy", "de");
    find(false, r"c\|d", "c", "c");
    find(false, r"c\|d", "d", "d");
    find(false, r"abc\|d*e", "xxxabceyy", "abce");
    not_find(false, r"abc\|d*e", "xxxabcceyy");
    find(false, r"abc\|d*e", "xxxabdeyy", "abde");
    find(false, r"abc\|d*e", "xxxabeyy", "abe");
    find(false, r"abc\|d*e", "xxxabdddeyy", "abddde");

    find(false, r"\(abc\)\|de", "xxxabcdeyy", "de");
    find(false, r"\(abc\)\|de", "xxxabceyy", "abce");
    find(false, r"x\(abc\)\|de", "xxxabceyy", "xabce");
    not_find(false, r"x\(abc\)\|de", "xxxabeyy");
    find(false, r"x\(abc\)+\|\(de\)*d", "xxxabcabcd", "xabcabcd");
    find(false, r"x\(abc\)+\|\(de\)*d", "xxxdedededx", "xdededed");
}

#[test]
fn lazy() {
    find(false, r"abc*", "xabccc", "abccc");
    find(false, r"abc*?", "xabccc", "ab");
    find(false, r"abc+?", "xabccc", "abc");
    find(false, r"abc*?d", "xabcccd", "abcccd");
    find(false, r"abc+?d", "xabcccd", "abcccd");
    find(false, r"abc+d", "xabcccd", "abcccd");
    find(false, r"a\(bcd\)+?bc", "abcdbcdbcd", "abcdbc");
    find(false, r"a\(bcd\)+bc", "abcdbcdbcd", "abcdbcdbc");
    find(false, r"ab*\|b", "abbbbbc", "abbbbb");
    find(false, r"ab*?\|b", "abbbbbc", "a");
    find(false, r"ab+?\|b", "abbbbbc", "ab");
    find(false, r"ab+?c", "abbbbbc", "abbbbbc");     // lazy back off
}

#[test]
fn former_bugs() {
    find(false, r"\(de\)*d", "dededede", "dededed");
    find(false, r"x\(abc\)+\|\(de\)*d", "xxxdededede", "xdededed");
}

fn get_report<'a>(re: &'a str, text: &'a str, ) -> Report {
    let tree = parse_tree(re, false).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let (path, start) = walk_tree(&tree, text).unwrap_or_else(|_| panic!("RE \"{}\" failed to parse", re)).unwrap_or_else(|| panic!("search unexpectedly failed"));
    crate::regexp::Report::new(&path, start, )
}       
fn check_report(report: &Report, expected: &str, pos: (usize, usize), bytes: (usize, usize), child_count: usize) {
    assert_eq!(report.found, expected);
    assert_eq!(report.pos, pos);
    assert_eq!(report.bytes, bytes);
    assert_eq!(report.subreports.len(), child_count);
}

//
// Reports test
//
// this could be more comprehensive, but regexp is not a real project
//
#[test]
fn reports() {
    // basic
    let report = get_report("asd", ".asd.");
    check_report(&report, "asd", (1, 4), (1, 4), 0);
    // basic, unicode
    let report = get_report("你好", ".你好.");
    check_report(&report, "你好", (1, 3), (1, 7), 0);
    // simple AND
    let report = get_report(r"ab\(cd\)ef", ".abcdef.");
    check_report(&report, "abcdef", (1, 7), (1, 7), 1);
    check_report(&report.subreports[0], "cd", (3, 5), (3, 5), 0);
    // simple AND unicode
    let report = get_report(r"ab\(你\)好ef", ".ab你好ef.");
    check_report(&report, "ab你好ef", (1, 7), (1, 11), 1);
    check_report(&report.subreports[0], "你", (3, 4), (3, 6), 0);
    assert!(report.get_by_name("fred").is_empty());
    // nested and with repetition
    let report = get_report(r"ab\(cd\(ef\)+\)+", ".abcdefefcd.");
    check_report(&report, "abcdefef", (1, 9), (1, 9), 1);
    check_report(&report.subreports[0], "cdefef", (3, 9), (3, 9), 2);
    check_report(&report.subreports[0].subreports[0], "ef", (5, 7), (5, 7), 0);
    check_report(&report.subreports[0].subreports[1], "ef", (7, 9), (7, 9), 0);
    // silent AND
    let report = get_report(r"ab\(?cd\)ef", ".abcdef.");
    check_report(&report, "abcdef", (1, 7), (1, 7), 0);
    // named
    let report = get_report(r"ab\(?<first>cd\(?<second>ef\)+\)+", ".abcdefefcd.");
    check_report(&report, "abcdefef", (1, 9), (1, 9), 1);
    check_report(&report.subreports[0], "cdefef", (3, 9), (3, 9), 2);
    check_report(&report.subreports[0].subreports[0], "ef", (5, 7), (5, 7), 0);
    check_report(&report.subreports[0].subreports[1], "ef", (7, 9), (7, 9), 0);
    assert!(report.get_by_name("fred").is_empty());
    let first = report.get_by_name("first");
    assert!(first.len() == 1);
    check_report(&first[0], "cdefef", (3, 9), (3, 9), 2);
    let second = report.get_by_name("second");
    assert!(second.len() == 2);
    check_report(&second[0], "ef", (5, 7), (5, 7), 0);
    check_report(&second[1], "ef", (7, 9), (7, 9), 0);
    let zeroth = report.get_by_name("");
    check_report(&zeroth[0], "abcdefef", (1, 9), (1, 9), 1);
    assert!(report.get_by_name("fake").is_empty());
    // all names
    let all = report.get_named();
    let zeroth = all.get("").unwrap();
    check_report(&zeroth[0], "abcdefef", (1, 9), (1, 9), 1);
    let first = all.get("first").unwrap();
    assert!(first.len() == 1);
    check_report(&first[0], "cdefef", (3, 9), (3, 9), 2);
    let second = all.get("second").unwrap();
    assert!(second.len() == 2);
    check_report(&second[0], "ef", (5, 7), (5, 7), 0);
    check_report(&second[1], "ef", (7, 9), (7, 9), 0);
    assert!(all.get("fake").is_none());
}

//
// error tests
//
fn e_check(alt: bool, re: &str, ecode: usize) {
    match parse_tree(re, false) {
        Ok(_) => panic!("Expected error {} parsing \"{}\", didn't get it", ecode, re),
        Err(error) => assert!(error.code == ecode, "Parsing \"{}\", expected error {}, found error {} ({})", re, ecode, error.code, error.msg),
    }
}
    
#[test]
fn errors() {
    e_check(false, r"abc\(de", 2);
    e_check(false, r"abc[de", 4);
    e_check(false, r"asd\)as", 5);
    e_check(false, r"\(?<asd\)", 7);
    e_check(false, r"\|sdf", 9);
    e_check(false, r"asd{as", 10);
    e_check(false, r"asd{4as", 12);
}

#[test]
fn alt_chars() {
    find(true, r"abc", "xabcd", "abc");
    find(true, r"abc ", "xabcd", "abc");
    find(true, "\"abc\"", "xabcd", "abc");
    find(true, r"'abc'", "xabcd", "abc");
    find(true, r"txt(abc)", "xabcd", "abc");
    find(true, r"ab\ cd", "xab cde", "ab cd");
    not_find(true, r"ab cd", "xab cde");
    find(true, r"ab c", "xabcde", "abc");

    find(true, r"ab*c", "xabbbbcd", "abbbbc");
    find(true, r"a\dc", "xa9cd", "a9c");
    find(true, r"a\d*c", "xacd", "ac");
}

#[test]
fn alt_or() {
    find(true, r"or('abc'
 	'def')", "xdefd", "def");
    find(true, r"or(a{3} y+ z )", "aayx", "y");
    find(true, r"or(a{3} y+ z )", "aaayyz", "aaa");
    find(true, r"or(a{3} y+ z )", "aazayyz", "z");
}

#[test]
fn alt_err() {
    e_check(false, r"or(abc def)", 2);
}
