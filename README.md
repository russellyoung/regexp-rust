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

**DONE!**

The last feature has been added: accepting input from stdin or a file.
It uses mutex to maintain a static string value. This may not be the best
way to do it - if I were starting over I'd pass a state containing the
current string down through the call tree, but retrofitting the working
program to do that would have been a real job, and this way works. It 
does have some less than ideal results though - the testing needs to be 
throttled to run single threaded, and interactive became a little more
complicated because it uses RE parsing to parse the command lines as 
well as the user input.

It is written and seems to be working. Besides standard regular
expressions I designed a new regular expression format that is both
more general and simpler than "regular" regular expressions. This was
done mainly as an exercise - I don't expect my new format to sweep the
internet, but who knows, I may adopt it myself. It is described below.

## _PROJECT SUMMARY_

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


## _Alternate Parser_

### Rationale

While trying to handle some of the idiosyncracies of traditional
regular expressions I realized that a more logically designed language
makes a lot of sense. The particulars of **or**, no way to group
characters save using an **AND** node, and **OR**s and repetitions
binding to single characters added comlexity, make it an illigically
designed language, and require complexity to implement that does not
provide sufficient payback (IMHO).

In addition, while trying to set up emacs to edit rust files, I ran
across some emacs source code where the writer developed a bunch of
macros to make long, complex regular expressions easier to write and
to understand. Making a new regular expression syntax would only
require a new front end, if the foundations and walk logic were
flexible enough to provide common tools, and (with a little work) they
turned out to have them.

So, here is a very brief intro to the alternate regular
expressions. If anyone cares to ask I can write up a better one. It
provides the following features:

 - Whitespace between units are ignored. This means indenting and new
   lines can be used to help make the meaning clearer
 - Characters are clumped together into string units. String units can
   have names and repetition counts added to them without needing to
   wrap them in **AND** nodes. They can also contain repetition counts
   internally, though these cannot have names assigned.
 - String units contain characters, special characters, and
   ranges. They can be written in several ways: 
   - **"..."** or **'...'** a unit surrounded with either single or
   double quotes is a string unit. This way inserting one of the quote
   characters is easier. Or, quote characters can be escaped using '\\'.
   - **txt(...)** Similar to the syntax for **and(...)** and **or(...)**,
     the **txt(...)** function can be used to make a string unit.
   - Finally, characters without any unit indicator are interpreted as
     string units. This makes writing them simple, but does have
     drawbacks: no whitespace characters can be embedded, unless
     preceded by '\\', and care must be taken that there is a space
     afterward to signify the termination. **or(abc def)** will not
     find the string "abc" **or** "def", it will report an error
     because the closing ')' of the **or()** will be interpreted as
     part of the string "def)". Also, using this implied notation, it
     is not possible to assign names or repeat counts to string
     units. For that the units must be enclosed.
 - **AND** nodes are created by enclosing them using the function
   notation **and(...)**. Inside the **AND** any units can be
   included. There is no comma or other punctioation required to
   separating two **AND** children.
 - **OR** nodes, like **AND** nodes, are written explicitly using the
   enclosure **or(...)**. This is a big win over traditional regular
   expressions that use "\|" as an infix function indicator.
 - **Names and repetition counts**: All units can have names or
   repetition counts. They are defined by trailing a unit with the
   proper code.
   - Repetition counts use the traditional notation: '?', '*', '+',
	 '{...}', optionally followed by '?' to indicate lazy evaluation.
   - Names are defined by including the name within "\<\>" brackets. To
     record a match without a name leave the brackets empty: "\<\>",
     define a name by including it inside: "\<name\>". If no brackets
     are included the match is not reported in the results structure.
   - The order in which the name and repetition count are given is
     significant. **_UNIT_(...)+\<name\>** will have a single named
     result containing one or more matches of UNIT;
     **_UNIT_(...)\<name\>+** will have one or more matches with the
     name "name"
 - **Definitions**: commonly used sequences can be defined by using
   the **def(...)** command. Once defined these can be used
   repeatedly, optionally with custom name and repeat count. These can
   also be included in files and read in by **use(...)**, allowing for
   a library of common custom pieces.
 
### _Final comments_

In any case, I'm ready to take a break from Rust. 2 programs, each one
rewritten probably 10 times, is a lot of work and a lot of time, and I
need to get back to other things. Still, it has been an eye-opening
experience, and when I get back to the US and look for work I can at
least consider positions that use Rust.


Even though it's almost done, comments are still welcomed and
encouraged. I promise to think about everything that comes in.

