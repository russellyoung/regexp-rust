# Project 2, Regular Expression Search

A couple weeks ago I finished my first Rust project,
[Tetrii](https://github.com/russellyoung/tetrii). I've used this as a
standard project in several languages - raw X Window, Python, Java,
Javascript and Node.js, even elisp.

The Rust version works, but as opposed to other languages it was a
poor choice. Writing tetrii requires graphics, async programming
(maybe threads), OO design (explicit or implicit), all in a
non-trivial but well-understood algorithm. In addition, throwing in
bells and whistles, there can be file handling, command line parsing,
and other feaures. I chose gtk for graphics, which seemed like the
most basic choice. Mabe other graphics crates are better integrated,
but gtk is closely tied to glib, which mainly involves working around
Rust's memory restrictions, rather than working within it. Most
structs either are wraped in or contain members wrapped in
**Rc<RefCell<_>>**, and so is effectively mutable whenever you want to
change it. Even so, the first design had to be completely redone as it
approached the first milestone, a single running board, because it
couldn't be forced to work.

That leads into this second project. I wrote a [regexp search program
in C](https://young-0.com/regexp) some time ago, and though I haven't
redone it since a little review brought to mind the design, and after
a few week's rest I decided to give it a go.

### _The current state_

## _RELEASE!_

I'm pleased to say it seems to be done. The basic program is working,
the support environment - testing and documentation - is done, to some
extent at least (I may add more tests). It even has some extra
features added, like a couple API functions to make accessing results
easier, and an interactive mode where a user can keep around a bunch
of regular expressions and strings to search, and experiment with
changing them, and even view the RE parse result and the tree walk.

Writing it has been moderately successful. I am a lot more comfortable
with Rust than I was. Rewriting the code about 10 times as I
understood new features will do that. Still, there is a lot I still
have to go. For one thing,I am still pretty hazy on the implementation
of lifetime management. Sure, in theory it seems pretty
straightforward, but getting the lifetime tags in the right places and
matching still makes little sense to me. My MO is to write what I
think is right, see what the compiler suggests, and then try to do
that. Even then I find too often it takes a long time, and looking at
the right way I still can't always figure out why. And I still haven't
dared look into macros, though there were a few places I could have
used them (the entire trace system should be macros and not functions).

Also, one thing I have not been able to do is combine all my Nodes and
Steps with traits. Whatever I tried worked well until the last couple
errors, which always seemed to be caused by the design. As a result
there is a lot of nearly duplicate code that I wasn't able to combine
into one base object, and almost all the Path methods are simply to
distribute the method call to the object that should handle it.

And googling documents to try to understand blockig problems I also
found that there is a lot more going on that I don't know about.
Despite its surface similarity to other languages, 

### _TODO_

The TODO list is amost gone. All that remains is the biggest and most
important one of all, refactoring to combine the Nodes and Steps into
a single superclass/trait that would eliminate the need for duplicate
code, switching methods in Path, and the distinctions between the
different Nodes and Steps.

And maybe writing a few more tests Peekable and Report are not being tested
yet. But it is hard to get too enthusiastic about it anymore.


### _Final comments_

In any case, I'm ready to take a break from Rust. 2 programs, each one
rewritten probably 10 times, is a lot of work and a lot of time, and I
need to get back to other things. Still, it has been an eye-opening
experience, and when I get back to the US and look for work I can at
least consider positions that use Rust.

Finally, these questions are left over from the previous README edition, and still seem relevant:

- Is wrapping my structs in enums a bad idea? I haven't seen this done elsewhere, usually Box is used. Is there some reason this would be discouraged?
- How about my trace() method? While it doesn't make a big difference here I don't want to evaluate the args unless they will be printed. This looks like a good place for a macro, right? That may be the next thing I try figuring out, though it looks kind of daunting.
- I think a well designed trait would let me make all the Path enums contents use the dyn trait rather than different objects. This would minimize the need for all those Path methofs that just distribute messages to their struct content. I looked into this briefly at one poin but ran into issues and wen back to the simpler way.

Comments welcomed and encouraged. I promise to think about everything that comes in.
