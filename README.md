# Project 2, Regular Expression Search

A couple weeks ago I finished my first Rust project, [Tetrii](https://github.com/russellyoung/tetrii). I've used this as a standard project in several languages - raw X Window, Python, Java, Javascript and Node.js, even elisp. 

The Rust version works, but as opposed to other languages it was a poor choice. Writing tetrii requires graphics, async programming (maybe threads), OO design (explicit or implicit), all in a non-trivial but well-understood algorithm. In addition, throwing in bells and whistles, there can be file handling, command line parsing, and other feaures. I chose gtk for graphics, which seemed like the most basic choice. Mabe other graphics crates are better integrated, but gtk is closely tied to glib, which mainly involves working around Rust's memory restrictions, rather than working within it. Most structs either are wraped in or contain members wrapped in **Rc<RefCell<_>>**, and so is effectively mutable whenever you want to change it. Even so, the first design had to be completely redone as it approached the first milestone, a single running board, because it couldn't be forced to work.

That leads into this second project. I wrote a [regexp search program in C](https://young-0.com/regexp) some time ago, and though I haven't redone it since a little review brought to mind the design, and after a few week's rest I decided to give it a go.

This is into the first try. The tree parser is almost written - it compiles, but I haven't run it yet. Still, I've hit my first roadblock, and if anyone reads this I'd appreciate opinions.

### _The problem_

I have a trait for a tree node and several structs that implement it: **CharsNode**, **AndNode**, **OrNode**, **RangeNode**,... The **AndNode** holds a **Vec<Box<dyn TreeNode>>**. The problem comes in when the parser hits a **"\|"** in the input. The algorithm is simple: if the preceding Node in the **AndNode** is an **OrNode** add the condition to it, otherwise move the preceding **Node** to the first slot of the **OrNode** and replace it in the **AndNode** list with the **OrNode**. Problem: the preceding item is a **Node**. The **Node** trait lets me see what it is but I can't figure out how to access it as an **OrNode** and not a **Node**.

### _Potential Solutions_

The simplest solution would probably be some way of casting the **Node** to an **OrNode**, probably in an **unsafe {}** block. So that is question 1: can this be done?

The second, rather inelegant, solution is to have a special case that recognizes an **OrNode** and holds a ref to it as an **OrNode**. This is doable, but ugly - the parsing stuff is all localized in one place, doing this would require moving some of it from a **parse()** function into the **AndNode.parse()** method, removing the logic from one location and spreading it out. Designwise I don't like this - splitting the logic up both makes it harder to understand and more likely to introduce bugs.

The third, which is probably what I will try first, is to use an enum for the **Node**s, rather than relying on the trait. That is, rather than **AndNode** holding its child nodes in a **Vec<Box<dyn Node>>** it will use an enum
**enum Node {Chars(CharsNode), And(AndNode), Or(OrNode), ...}**  
This not only makes identifying a **Node** easy, but should allow me to get a reference to any **Node**. Should this work?
