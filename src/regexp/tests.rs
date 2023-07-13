use crate::regexp::*;
use std::io::Write;
use std::sync::Mutex;

// This is needed because the input string is stored statically, and so the test breaks if it is run multi-thread.
pub static LOCK: Mutex<usize> = Mutex::new(0);

//
// Initial tests are basic sanity tests for the tree parser. They are relatively simple because the
// search tests (TODO) will provide more complete testing. These are intended mainly as a sanity check
// make sure that the parsing basically works.
//

#[test]
fn peekable() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
            && peek_4[3].is_none());
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
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    assert!(chars.next().is_none(), "Failed to consume test string");
}

//
// parser tests
//
fn make_chars_string(string: &str) -> Node {
        Node::Chars(CharsNode{string: string.to_string(), limits: Limits::default(), named: None, name_outside: false})
}
fn make_chars_single(ch: char, min: usize, max: usize, lazy: bool) -> Node {
    Node::Chars(CharsNode{string: ch.to_string(), limits: Limits{min, max, lazy}, named: None, name_outside: false})
}

fn make_root (min: usize, max: usize, lazy: bool) -> Node { make_and(min, max, lazy, Some(""))}

fn make_and (min: usize, max: usize, lazy: bool, name: Option<&str>) -> Node {
    let named = name.map(|n| n.to_string());
    Node::And(AndNode{nodes: Vec::<Node>::new(), limits: Limits{min, max, lazy}, named, anchor: false, name_outside: false})
}
fn make_or() -> Node {
    Node::Or(OrNode{nodes: Vec::<Node>::new(), limits: Limits::default(), named: None, name_outside: false})
}

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
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("abcd"));
    assert_eq!(node, parse_tree("abcd", false).unwrap());
}

#[test]
fn test_string_embedded_reps_greedy() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("ab"));
    node.push(make_chars_single('c', 0, 1, false));
    node.push(make_chars_string("de"));
    node.push(make_chars_single('f', 1, EFFECTIVELY_INFINITE, false));
    node.push(make_chars_string("gh"));
    node.push(make_chars_single('i', 0, EFFECTIVELY_INFINITE, false));
    assert_eq!(node, parse_tree("abc?def+ghi*", false).unwrap());
}
              
#[test]
fn test_string_embedded_reps_lazy() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("ab"));
    node.push(make_chars_single('c', 0, 1, true));
    node.push(make_chars_string("de"));
    node.push(make_chars_single('f', 1, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string("gh"));
    node.push(make_chars_single('i', 0, EFFECTIVELY_INFINITE, true));
    node.push(make_chars_string("jk"));
    assert_eq!(node, parse_tree("abc??def+?ghi*?jk", false).unwrap());
}

#[test]
fn or_with_chars_bug() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    let mut node = make_root(1, 1, false);
    node.push(make_chars_string("ab"));
    let mut or_node = make_or();
    or_node.push(make_chars_string("c"));
    or_node.push(make_chars_string("d"));

    node.push(or_node);
    node.push(make_chars_string("ef"));
    assert_eq!(node, parse_tree(r"abc\|def", false).unwrap());
}

fn find<'a>(alt: bool, re: &'a str, text: &'a str, expected: &'a str) {
    print!("RUNNING '{}' '{}'... ", re, text);
    std::io::stdout().flush().unwrap();
    let tree = parse_tree(re, alt).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let path = walk_tree(&tree, text, "")
        .unwrap_or_else(|err| panic!("Expected \"{}\", got error '{}'", expected, err))
        .unwrap_or_else(|| panic!("Expected {}, found none", expected));
    assert_eq!(path.matched_string(), expected, "re \"{}\" expected \"{}\", found \"{}\"", re, expected, path.matched_string());
    println!("OK");
}       
        
fn not_find<'a>(alt: bool, re: &'a str, text: &'a str) {
    let tree = parse_tree(re, alt).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    assert!(walk_tree(&tree, text, "").unwrap().is_none(), "re \"{}\" expected no match, found one", re);
}       

//
// walk tests
//
#[test]
fn simple_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(false, "abc", "abcd", "abc");
    find(false, "bcd", "abcd", "bcd");
    find(false, "bcd", "abcde", "bcd");
    not_find(false, "bcd", "abde");
}

#[test]
fn rep_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(false, "abc", "ab你好abcd", "abc");
    find(false, "你好", "ab你好abcd", "你好");
    find(false, "你好you-all", "ab你好you-allabcd", "你好you-all");
    find(false, "a.*a", "qqab你好abcd", "ab你好a");
    find(false, r"是很*好", "这是很很很好",  "是很很很好");
}

    
#[test]
fn special_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    
#[test]
fn set_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(false, r"[abc]+", "xabacaacd", "abacaac");
    find(false, r"z[abc]*z", "abzzcd", "zz");
    find(false, r"z[abc]*z", "abzaabczcd", "zaabcz");
    not_find(false, r"z[abc]*z", "abzaabcdzcd");
    find(false, r"z[a-m]*z", "abzabclmzxx", "zabclmz");
    find(false, r"[a-mz]+", "xyzabclmzxx", "zabclmz");
    find(false, r"[a\wx-z]+", "qa x\ty\nz q", "a x\ty\nz ");
}

// #[test]
fn non_set_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(false, "a[^hgf]*", "aabcdefghij", "aabcde");
    find(false, "a[^e-m]*", "aabcdefghij", "aabcd");
    find(false, "a[^-e-m]*", "xab-cdefghij", "ab");
    not_find(false, "[^abcd]+", "abcdab");
    find(false, r"[^\d\l]*", "ABCd123", "ABC");
    find(false, r"[^\d\l]*", "ABCD123", "ABCD");
}

#[test]
fn basic_or() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(false, r"\(de\)*d", "dededede", "dededed");
    find(false, r"x\(abc\)+\|\(de\)*d", "xxxdededede", "xdededed");
}

fn report_test<'a>(re: &'a str, text: &'a str, alt: bool, func: fn(&Report)) {
    let tree = parse_tree(re, alt).unwrap_or_else(|msg| panic!("Parse failed for re \"{}\": {}", re, msg));
    let path = walk_tree(&tree, text, "").unwrap_or_else(|_| panic!("RE \"{}\" failed to parse", re)).unwrap_or_else(|| panic!("search unexpectedly failed"));

    let report = Report::new(&path);
    func(&report);
}       

fn check_report(report: &Report, expected: &str, pos: (usize, usize), bytes: (usize, usize), child_count: usize) {
    assert_eq!(report.string(), expected);
    assert_eq!(report.char_pos(), pos);
    assert_eq!(report.byte_pos(), bytes);
    assert_eq!(report.subreports.len(), child_count);
}

//
// Reports test
//
// this could be more comprehensive, but regexp is not a real project
//
// When I changed Report to use a reference rather than make a new String this wouldn't compile
// because PATH and TREE belonged to get_report() (the original name of report_test()). There was
// no problem in the code, just in the tests. I played around with the organization and passing
// args without success, so finally I figured out rather than passing the Report back to the
// checks I could pass the checks to the Report.
#[test]
fn reports() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    // basic
    report_test("asd", ".asd.", false, |report: &Report| {
        check_report(report, "asd", (1, 4), (1, 4), 0);
    });
    // basic, unicode
    report_test("你好", ".你好.", false, |report: &Report| {
        check_report(report, "你好", (1, 3), (1, 7), 0); }
    );
    // simple AND
    report_test(r"ab\(cd\)ef", ".abcdef.", false, |report: &Report| {
        check_report(report, "abcdef", (1, 7), (1, 7), 1);
        check_report(&report.subreports[0], "cd", (3, 5), (3, 5), 0);
    });
    // simple AND unicode
    report_test(r"ab\(你\)好ef", ".ab你好ef.", false, |report: &Report| {
        check_report(report, "ab你好ef", (1, 7), (1, 11), 1);
        check_report(&report.subreports[0], "你", (3, 4), (3, 6), 0);
        assert!(report.get_by_name("fred").is_empty());
    });

    // nested and with repetition
    report_test(r"ab\(cd\(ef\)+\)+", ".abcdefefcd.", false, |report: &Report| {
        check_report(report, "abcdefef", (1, 9), (1, 9), 1);
        check_report(&report.subreports[0], "cdefef", (3, 9), (3, 9), 2);
        check_report(&report.subreports[0].subreports[0], "ef", (5, 7), (5, 7), 0);
        check_report(&report.subreports[0].subreports[1], "ef", (7, 9), (7, 9), 0);
    });
    // silent AND
    report_test(r"ab\(?cd\)ef", ".abcdef.", false, |report: &Report| {
        check_report(report, "abcdef", (1, 7), (1, 7), 0);
    });
    // named
    report_test(r"ab\(?<first>cd\(?<second>ef\)+\)+", ".abcdefefcd.", false, |report| {
        check_report(report, "abcdefef", (1, 9), (1, 9), 1);
        check_report(&report.subreports[0], "cdefef", (3, 9), (3, 9), 2);
        check_report(&report.subreports[0].subreports[0], "ef", (5, 7), (5, 7), 0);
        check_report(&report.subreports[0].subreports[1], "ef", (7, 9), (7, 9), 0);
        assert!(report.get_by_name("fred").is_empty());
        let first = report.get_by_name("first");
        assert!(first.len() == 1);
        check_report(first[0], "cdefef", (3, 9), (3, 9), 2);
        let second = report.get_by_name("second");
        assert!(second.len() == 2);
        check_report(second[0], "ef", (5, 7), (5, 7), 0);
        check_report(second[1], "ef", (7, 9), (7, 9), 0);
        let zeroth = report.get_by_name("");
        check_report(zeroth[0], "abcdefef", (1, 9), (1, 9), 1);
        assert!(report.get_by_name("fake").is_empty());
        // all names
        let all = report.get_named();
        let zeroth = all.get("").unwrap();
        check_report(zeroth[0], "abcdefef", (1, 9), (1, 9), 1);
        let first = all.get("first").unwrap();
        assert!(first.len() == 1);
        check_report(first[0], "cdefef", (3, 9), (3, 9), 2);
        let second = all.get("second").unwrap();
        assert!(second.len() == 2);
        check_report(second[0], "ef", (5, 7), (5, 7), 0);
        check_report(second[1], "ef", (7, 9), (7, 9), 0);
        assert!(all.get("fake").is_none());
    });
}
//
// error tests
//
fn e_check(alt: bool, re: &str, ecode: usize) {
    match parse_tree(re, alt) {
        Ok(_) => panic!("Expected error {} parsing \"{}\", didn't get it", ecode, re),
        Err(error) => assert!(error.code == ecode, "Parsing \"{}\", expected error {}, found error {} ({})", re, ecode, error.code, error.msg),
    }
}
#[test]
fn errors() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    e_check(false, r"abc\(de", 1);
    e_check(false, r"\(?<asd\)", 2);
    // 3 should not happen
    e_check(false, r"\|sdf", 4);
    // 5 should not happen
    e_check(false, r"asd\)as", 6);
    e_check(false, r"asd{as", 7);
    e_check(false, r"asd{4as", 7);
    e_check(false, r"asd{4,x", 8);
    e_check(false, r"abc[de", 9);
}

#[test]
fn alt_chars() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
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
    find(true, r"a[vx-z]*c", "xavxyzcd", "avxyzc");
    find(true, r"a[vx-z]*c", "xacd", "ac");
    find(true, r"a[^vx-z]", "avaxayazawa", "aw");
    find(true, r"'abcd'+", "xabcdabcdabc", "abcdabcd");
    find(true, r"'abcd*'", "xabcabcdabc", "abc");

    find(true, r"a\l*", "abcDefg", "abc");
    find(true, r"a\u*", "aBCDEfg", "aBCDE");
    find(true, r"a\x*", "a09bcDefg", "a09bcDef");
    find(true, r"a\o*", "a0123456789", "a01234567");
    find(true, r"a\a*", "a.+你好", "a.+");
}

#[test]
fn alt_or() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(true, r"or('abc'
 	'def')", "xdefd", "def");
    find(true, r"or(a{3} y+ z )", "aayx", "y");
    find(true, r"or(a{3} y+ z )", "aaayyz", "aaa");
    find(true, r"or(a{3} y+ z )", "aazayyz", "z");
    find(true, r"or(a* y+ z )yz", "aayyyz", "yyyz");
}

#[test]
fn alt_def() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    find(true, "def(xx: 'xyz') w get(xx)", "vwxyz", "wxyz");
    find(true, "def(aa: 'xyz') w get(aa) get(aa)", "vwxyzxyz", "wxyzxyz");
    find(true, "use(src/regexp/test.re) a get(a)", "aabcdef", "abcd");
    find(true, "use(src/regexp/test.re) a get(z)+", "aawxyzwx", "awxyzwx");
    find(true, r"def(a: x ){3} get(a) b get(a){4}", "zxxxxbxxxxxx", "xxxbxxxx");
}

#[test]
fn alt_err() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    e_check(true, "\"asd", 102);
    e_check(true, r"and(abc def)", 104);
    // the OR reads the trailing ')' for the wrapping AND node, which is why this is not 105
    e_check(true, r"or(abc def)", 104);
    e_check(true, r"or(abc or(def ", 105);
    e_check(true, r"get() ", 106);
    e_check(true, r"get(a() ", 107);
//    e_check(true, r"get(a) ", 108);
    e_check(true, r"def(snippet_a: aa get(snippet_b)zz ) def(snippet_b: bb get(snippet_a) yy ) abcd get(snippet_a) wxyz ", 109);
    e_check(true, r"and(asd )<xy ", 110);
    e_check(true, r"def(asd)", 111);
    e_check(true, r"def(asd:)", 112);
    e_check(true, r"use(asd()", 113);
    e_check(true, r"use(no-such-file)", 114);
}
#[test]
fn runtime_error() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    // infinite loop test
    if let Ok(tree) = parse_tree(r"and('x'*)*", true) {
        match walk_tree(&tree, "abccc", "") {
            Err(e) if e.code == 200 => (),
            _ => panic!("Expected infinite loop, didn't get it"),
        }
    } else { panic!("Parse failed for infinite loop test"); }
}


#[test]
// check that X*<NAME> and X<NAME>* behave right
fn alt_report() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    report_test(r"a and('bc'<n1>* '你好'*<n2>)", "xabcbc你好你好", true, |report| {
        check_report(report, "abcbc你好你好", (1, 10), (1, 18), 3);
        check_report(&report.subreports[0], "bc", (2, 4), (2, 4), 0);
        check_report(&report.subreports[1], "bc", (4, 6), (4, 6), 0);
        check_report(&report.subreports[2], "你好你好", (6, 10), (6, 18), 0);
        let n1 = report.get_by_name("n1");
        assert_eq!(n1.len(), 2, "<n>* test failed");
        let n2 = report.get_by_name("n2");
        assert_eq!(n2.len(), 1, "*<n> test failed");
    });
} 

#[test]
fn from_file() {
    let mut x = LOCK.lock().unwrap();
    *x += 1;
    print!("RUNNING file input test");
    std::io::stdout().flush().unwrap();
    let tree = parse_tree("XXX", false).unwrap_or_else(|msg| panic!("Parse failed for re \"XXX\": {}", msg));
    let path = walk_tree(&tree, "", "README")
        .unwrap_or_else(|err| panic!("Expected \"XXX\", got error '{}'", err))
        .unwrap_or_else(|| panic!("Expected \"XXX\", found none"));
    assert_eq!(path.matched_string(), "XXX", "expected \"XXX\", found \"{}\"", path.matched_string());
    println!("OK");
}
