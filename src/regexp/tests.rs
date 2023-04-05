use crate::regexp::*;
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

fn make_chars_string(string: &'static str) -> Node {
        Node::Chars(CharsNode{string: string.to_string(), lims: Limits{min: 1, max: 1, lazy: false}})
}
fn make_chars_single(string: &'static str, min: usize, max: usize, lazy: bool) -> Node {
    Node::Chars(CharsNode{string: string.to_string(), lims: Limits{min, max, lazy}})
}
fn make_special(special: char, min: usize, max: usize, lazy: bool) -> Node {
    Node::SpecialChar(SpecialCharNode {special, lims: Limits {min, max, lazy}})
}

fn make_root<'a> (min: usize, max: usize, lazy: bool) -> Node { make_and(min, max, lazy, Some(""))}

fn make_and<'a> (min: usize, max: usize, lazy: bool, name: Option<&'a str>) -> Node {
    let named = {if let Some(n) = name {Some(n.to_string())} else {None}};
    Node::And(AndNode{nodes: Vec::<Node>::new(), lims: Limits{min, max, lazy}, named, anchor: false})
}
fn make_or() -> Node {
    Node::Or(OrNode{nodes: Vec::<Node>::new(), lims: Limits::default()})
}

fn make_set(not: bool, targets: Vec<Set>, min: usize, max: usize, lazy: bool) -> Node {
    Node::Set(SetNode{not, targets, lims: Limits{min, max, lazy}})
}
fn push_sets(set_node: &mut SetNode, sets: &mut Vec<Set>) {
    set_node.targets.append(sets);
}

fn make_or_limits(node: &mut OrNode) { node.lims = Limits { min: 0, max: node.nodes.len() - 1, lazy: false }; }

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
// parse tests
//
#[test]
fn test_string_simple() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("abcd"));
    assert_eq!(node, parse_tree("abcd").unwrap());
}

#[test]
fn test_string_embedded_reps() {
    let mut node = make_root(1, 1, false);
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
    let mut node = make_root(1, 1, false);
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
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("abc"));
    node.push(make_special('.', 1, 1, false));
    node.push(make_chars_string("def", ));
    node.push(make_special('d', 0, 1, false));
    node.push(make_chars_string("gh", ));
    node.push(make_special('.', 1, 3, false));
    node.push(make_chars_string("ij", ));
    println!("{:#?}", parse_tree(r"abc.def\d?gh").unwrap());
    assert_eq!(node, parse_tree(r"abc.def\d?gh.{1,3}ij").unwrap());
}
    
#[test]
fn or_with_chars_bug() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("ab"));
    let mut or_node = make_or();
    or_node.push(make_chars_string("c"));
    or_node.push(make_chars_string("d"));
    make_or_limits(or_node.mut_or_ref());
    node.push(or_node);
    node.push(make_chars_string("ef"));
    assert_eq!(node, parse_tree(r"abc\|def").unwrap());
}

#[test]
fn set_basic() {
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("ab"));
    let targets = vec![Set::RegularChars("cde".to_string()),];
    node.push(make_set(false, targets, 0, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_string("fg"));
    let targets = vec![Set::RegularChars("h".to_string()),
                       Set::Range('i', 'k'),
                       Set::RegularChars("lm".to_string()),
                       Set::SpecialChar('d')];
    node.push(make_set(true, targets, 1, 1, false));
    assert_eq!(node, parse_tree(r"ab[cde]*fg[^hi-klm\d]").unwrap());
}


fn find<'a>(re: &'a str, text: &'a str, expected: &'a str) {
    let tree = parse_tree(re).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let (path, _, _) = walk_tree(&tree, text).unwrap_or_else(|_| panic!("Expected \"{}\", didn't find anything", expected)).unwrap();
    assert_eq!(path.matched_string(), expected, "re \"{}\" expected \"{}\", found \"{}\"", re, expected, path.matched_string());
}       
        
fn not_find<'a>(re: &'a str, text: &'a str) {
    let tree = parse_tree(re).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    assert!(walk_tree(&tree, text).unwrap().is_none(), "re \"{}\" expected no match, found one", re);
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
    find(r"是很*好", "这是很很很好",  "是很很很好");
}

    
#[test]
fn special_chars() {
    find("ab.de", "aabcdef", "abcde");
    find("ab.*de", "abcdedefg", "abcdede");
    not_find("cde.", "abcde");
    find(".*", "ab1234fg", "ab1234fg");
    find(r"\d+", "ab1234fg", "1234");
    find(r"\d*", "ab1234fg", "");
    find(r"b\d*", "ab1234fg", "b1234");
    not_find(r"xxx$", "abcxxxy");
    find(r"xxx$", "abcxxx", "xxx");
    find(r"xxx$z", "abcxxx$zx", "xxx$z");
    find(r"^abc", "abcdef", "abc");
    not_find(r"^abc", "xabcdef");
    find(r"a^bc", "xa^bcdef", "a^bc");
    find(r"a\d+\l+", "aba123Ba123bcD", "a123bc");
    find(r"\a\u+", "你好abCD没有", "bCD");
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
    // this one caught a bug
    find(r"x\(abc\)+\|\(de\)*d", "xxxdededede", "xdededed");
}

#[test]
fn lazy() {
    find(r"abc*", "xabccc", "abccc");
    find(r"abc*?", "xabccc", "ab");
    find(r"abc+?", "xabccc", "abc");
    find(r"abc*?d", "xabcccd", "abcccd");
    find(r"abc+?d", "xabcccd", "abcccd");
    find(r"abc+d", "xabcccd", "abcccd");
    find(r"a\(bcd\)+?bc", "abcdbcdbcd", "abcdbc");
    find(r"a\(bcd\)+bc", "abcdbcdbcd", "abcdbcdbc");
}


fn get_report<'a>(re: &'a str, text: &'a str, ) -> Report {
    let tree = parse_tree(re).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let (path, start, b_start) = walk_tree(&tree, text).unwrap_or_else(|_| panic!("RE \"{}\" failed to parse", re)).unwrap();
    crate::regexp::Report::new(&path, start, b_start)
}       
fn check_report(report: &Report, expected: &str, pos: (usize, usize), bytes: (usize, usize), child_count: usize) {
    assert_eq!(report.found, expected);
    assert_eq!(report.pos, pos);
    assert_eq!(report.bytes, bytes);
    assert_eq!(report.subreports.len(), child_count);
}

// this could be more comprehensive, but regexp is not a real project
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

fn e_check(re: &str, ecode: usize) {
    match parse_tree(re) {
        Ok(_) => panic!("Expected error parsing \"{}\", didn't get it", re),
        Err(error) => assert!(error.code == ecode, "Parsing \"{}\", expected error {}, found error {} ({})", re, ecode, error.code, error.msg),
    }
}
    
#[test]
fn errors() {
    e_check(r"abc\(de", 2);
    e_check(r"abc[de", 4);
    e_check(r"asd\)as", 5);
    e_check(r"asd{as", 10);
    e_check(r"asd{4as", 12);
}

