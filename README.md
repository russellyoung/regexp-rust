# Project 2, Regular Expression Search

A couple weeks ago I finished my first Rust project, [Tetrii](https://github.com/russellyoung/tetrii). I've used this as a standard project in several languages - raw X Window, Python, Java, Javascript and Node.js, even elisp. 

The Rust version works, but as opposed to other languages it was a poor choice. Writing tetrii requires graphics, async programming (maybe threads), OO design (explicit or implicit), all in a non-trivial but well-understood algorithm. In addition, throwing in bells and whistles, there can be file handling, command line parsing, and other feaures. I chose gtk for graphics, which seemed like the most basic choice. Mabe other graphics crates are better integrated, but gtk is closely tied to glib, which mainly involves working around Rust's memory restrictions, rather than working within it. Most structs either are wraped in or contain members wrapped in **Rc<RefCell<_>>**, and so is effectively mutable whenever you want to change it. Even so, the first design had to be completely redone as it approached the first milestone, a single running board, because it couldn't be forced to work.

That leads into this second project. I wrote a [regexp search program in C](https://young-0.com/regexp) some time ago, and though I haven't redone it since a little review brought to mind the design, and after a few week's rest I decided to give it a go.

### _The current state_

I'm pleased to say it is basically working. "Basically" here means the main functions are working, at least as far as I've tested them. The main functions are AND, OR, CHAR_MATCH, SPECIAL_MATCH (\N, ', etc.), and SET ([a-z], etc.) all work. Unicode seems to work, at least simple stuff - I know it can get complicated, but basic simple unicode strings (containing hanzi) While it is not fully tested, since the last few bugs were found all tests seems to work on the first try, so while there may still be minor bugs in the code the design looks solid.

### _TODO_

There is still a good-sized TODO list:

- implement lazy evaluation
- implement named capture groups
- add more special chars - currently only has '.' and '\N' (numeric), should add ^, $, whitespace, uppercase, lowercase...
- REFACTOR (most important). Especially in WALK, there is a lot of repeat code that can probably be replaced using traits properly. During development I intentionally did not think much about this. For one thing I was concentrating more on the Rust features needed to implement the types one by one, and I wasn't sure how much overlap there would be (it turned out to be a lot).

It would be really helpful to get some comments on the current design, and my thoughts for using traits. Other things I wonder about:

- Is wrapping my structs in enums a bad idea? I haven't seen this done elsewhere, usually Box is used. Is there some reason this would be discouraged?
- How about my trace() method? While it doesn't make a big difference here I don't want to evaluate the args unless they will be printed. This looks like a good place for a macro, right? That may be the next thing I try figuring out, though it looks kind of daunting.
- I think a well designed trait would let me make all the Path enums contents use the dyn trait rather than different objects. This would minimize the need for all those Path methofs that just distribute messages to their struct content. I looked into this briefly at one poin but ran into issues and wen back to the simpler way.
- Maybe moving Limits from the Steps to the Path would simplify things.
- If I do unify the Step objects with a Walk trait I wonder what that does to my trace system. I guess the messages could be added as functions in the trait, for instance "trace_enter_walk()"
- lazy evaluation is very similar to greedy, it differs only in the initial walk is to the minimum rather than the maximum, and the backoff() method does a step rather than pops off a step. I suppose supplying whole new methods for it would work, but there must be a cleverer way to do it.

Comments welcomed and encouraged. I promise to think about everything that comes in.
