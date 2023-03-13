# Project 2, Regular Expression Search

A couple weeks ago I finished my first Rust project, [Tetrii](https://github.com/russellyoung/tetrii). I've used this as a standard project in several languages - raw X Window, Python, Java, Javascript and Node.js, even elisp. 

The Rust version works, but as opposed to other languages it was a poor choice. Writing tetrii requires graphics, async programming (maybe threads), OO design (explicit or implicit), all in a non-trivial but well-understood algorithm. In addition, throwing in bells and whistles, there can be file handling, command line parsing, and other feaures. I chose gtk for graphics, which seemed like the most basic choice. Mabe other graphics crates are better integrated, but gtk is closely tied to glib, which mainly involves working around Rust's memory restrictions, rather than working within it. Most structs either are wraped in or contain members wrapped in **Rc<RefCell<_>>**, and so is effectively mutable whenever you want to change it. Even so, the first design had to be completely redone as it approached the first milestone, a single running board, because it couldn't be forced to work.

That leads into this second project. I wrote a [regexp search program in C](https://young-0.com/regexp) some time ago, and though I haven't redone it since a little review brought to mind the design, and after a few week's rest I decided to give it a go.

### _The current state_

The parser runs fine, and there are tests to handle some simple cases. More need to be added, but most testing will do full searching, the parser tree tests are mainly sanity tests to point the way if something is really wrong.

The next step is to walk the tree. I'm pretty sur ehow to do it, but still need to think a little bit about how to return the state when a particular node succeeds.

To me this feels much more like a Rust program than Tetrii did. In particular, it starts to make use of features like traits, enums, and distinguishes between mutable and immutable. I'd be curious to hear what others think.