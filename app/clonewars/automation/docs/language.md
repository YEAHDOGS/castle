# The Castle script language

The whole language, version 1. The compiler is [`castled/src/script.rs`](../../../../castled/src/script.rs).
It is the source of truth: when this page and the compiler disagree, the compiler wins, and
this page is wrong.

- [At a glance](#at-a-glance)
- [How a script is read](#how-a-script-is-read)
- [Names](#names)
- [Selectors](#selectors)
- [Text, values and fields](#text-values-and-fields)
- [Steps](#steps): [pages](#pages), [reading](#reading), [changing rows](#changing-rows),
  [summing up](#summing-up), [results](#results), [the log and pictures](#the-log-and-pictures),
  [checks and guards](#checks-and-guards), [Python](#python)
- [Numbers in text](#numbers-in-text)
- [Amounts and units](#amounts-and-units)
- [When a step fails](#when-a-step-fails)
- [What the compiler says](#what-the-compiler-says)
- [What a script becomes](#what-a-script-becomes)
- [Limits](#limits)

## At a glance

```text
castle 1                                          the version (optional, first line)
# a comment                                       a whole line starting with #
website shop.example.com                          open the browser on a site (once)

element search box = input[name="q"]              name one thing on the page
list results = .product                           name a repeating thing...
  field title = .title                            ...and what to take from each one
  field price = .price
  field link  = a @href                           an attribute instead of the text
  field id    = @data-id                          an attribute of the item itself

go to shop.example.com/deals                      open another page
type "motor oil" into search box                  type, a key at a time
click search button                               click
press enter  /  press enter in search box         a key
wait for results  /  wait 5 seconds               until it shows, or for a while
scroll down  /  scroll to the bottom              scroll once, or until the page stops growing

read results                                      every item into rows
read heading as page title                        one element's text into a value

keep results where title has "0W-20"              keep the rows that pass a test
drop results where price is empty                 drop the rows that pass it
keep results unique by id                         the first row of each id
measure results title in quarts as quarts         an amount read out of text
divide results price by quarts as price per quart a new field: a field / a field or number
multiply results price by 1.08 as with tax        a new field: a field * a field or number
sort results by price lowest first                or highest first

average results price as average price           also sum, min, max, median
lowest results price as best deal                 the whole cheapest row (also highest)
count results as found                            how many rows

post results to castle                            rows or a value into Castle's database
post best deal to castle as "best deal today"     under a dataset name of your own
track average price lower is better               a number kept day after day
say "{found} found, {average price} on average"   a line in the run's log
screenshot "the results"                          a picture of the page, kept with the run

fail if results is empty "nothing came back"      end the run when a list or value is empty
fail if sold out is shown "it is sold out"        end the run when the page shows something
pause whenever robot check is shown "answer it"   from here on: wait for a person
fail whenever error page is shown "site is down"  from here on: end the run

python:                                           Python, for what the language cannot say
    run.say("hello from Python")
```

## How a script is read

- **One step per line**, read top to bottom. There are no loops and no ifs. A script is a
  straight path; when a page might not go the way you expect, a [check](#checks-and-guards)
  ends the run or waits for a person.
- **Blank lines** are ignored. Use them to group steps.
- **Comments** are whole lines whose first character (after spaces) is `#`. A `#` later in
  a line is not a comment: `element box = #search` is the selector `#search`.
- **Indenting** means something in two places only: a list's `field` lines, and the body of
  `python:`. Anything else indented is a problem ("only a list's fields are indented").
- **Words are not case-sensitive.** `Click Search Button` is `click search button`. Quoted
  text and selectors keep their case.
- **Quoted text** goes in straight double quotes `"..."`. Curly quotes `“...”`, which some
  editors type for you, work too. `\"` puts a quote inside quoted text, `\\` a backslash.
- **The version line**, `castle 1`, is optional. If it is there, it is the first step.
  A Castle that reads version 1 refuses `castle 2`, so a script never runs on a Castle that
  would misread it.
- **The whole script is checked before anything runs.** Every problem comes back at once,
  with its line. Nothing starts until there are none. The editor checks as you type.

## Names

Elements, lists, list fields and values all have names you choose.

- A name is **words**: letters, digits, spaces, `-` and `_`, starting with a letter, up to
  60 characters. `search box`, `price per quart`, `step-2`, `item_id` are all names.
- `as` and `where` cannot be words of a name. They join the parts of a step
  (`average results price as average price`, `keep results where price is below 10`).
- **Declare before use.** An `element` or `list` line comes before the first step that
  uses it. A value exists from the step that makes it (`... as <name>`) onward.
- **Longest name first.** With `element search` and `element search button` both declared,
  `click search button` clicks the button. The compiler always takes the longest name it
  knows at that point in the line.
- Elements and lists share one set of names: a list cannot be called what an element is.
  A step's answer (`as <name>`) cannot reuse an element's or a list's name either, but it
  can reuse a value's name, which replaces the old value.
- A list's fields are its own: two lists can each have a `price`.

## Selectors

What comes after `=` in an `element`, `list` or `field` line is a **selector**: how the
browser finds it on the page. Castle passes it to the browser as it is, so anything
[Playwright](https://playwright.dev/python/docs/locators) understands works.

| Selector | Finds |
|---|---|
| `#search` | the element with `id="search"` |
| `.price` | elements with the class `price` |
| `input[name="q"]` | an `<input>` whose `name` is `q` |
| `button[aria-label="Search"]` | a button by its label for screen readers |
| `[data-item-id]` | anything with a `data-item-id` attribute |
| `h3 a` | a link inside an `<h3>` |
| `text "Robot or human?"` | the element showing these words |
| `label "Email"` | the form field labelled "Email" |
| `placeholder "Search Walmart"` | the field with this grey hint text |
| `title "Search"` | the element whose tooltip (`title`) is this |
| `text=Sign in` | Playwright's own text selector |
| `xpath=//table//tr[2]` | XPath |

The four plain forms (`text`, `label`, `placeholder`, `title`, then the words in quotes)
are Castle's. They match whole or part words, case aside, the way Playwright's `get_by_text`
and friends do.

**When a selector matches more than one thing**, an element step uses the first match on the
page and waits for that one to be visible. If the first match is hidden (a desktop menu kept
for phones, say), make the selector narrower. A list uses every match, one row each.

**Inside a list**, a field's selector looks inside one item only. `field title = .title`
reads the `.title` inside each `.product`, not the first one on the page. A field whose
selector finds nothing in an item is empty for that item. An item where every field is empty
is left out.

**Attributes.** End a field's selector with `@<attribute>` to read that attribute instead
of the text: `a @href`, `img @src`, `meta @content`. `@href` and `@src` come back as full
addresses, even when the page writes them relative. `@data-item-id` alone (nothing before
the `@`) reads the item's own attribute. Attributes are letters, digits, `-`, `_` and `:`.

**The text of a field** is what the page shows, with runs of spaces and line breaks made
single spaces. Text that is on the page for screen readers only (hidden from the eye) comes
back too: when the shown text is empty, Castle takes the written text.

The [cookbook](cookbook.md#finding-a-selector) shows how to find selectors with the
browser's own developer tools, and what makes one last.

## Text, values and fields

Quoted text in `type`, `say` and the messages of `fail` and `pause` can have pieces in
braces, which are filled in when the step runs:

- **`{name}`**, in lower case or mixed case: a **value** of the script, made by an earlier
  step that ends in `as <name>`, or a list (written as "12 rows"). It must exist by that
  line, or the compiler says so.
- **`{NAME}`**, in capitals, digits and `_` only: a **field the bot fills in**, such as
  `{EMAIL}` or `{API_KEY}`. Add a field with exactly that name under *What each bot fills
  in*. Each bot has its own value, and secret ones are never shown again after they are
  saved. Castle refuses to start a script whose fields the automation does not have.

How values are written into text:

| Value | Written as |
|---|---|
| a whole number | `1240` (a count: `1,240`) |
| any other number | `6.39` (two decimals, with thousands commas) |
| a number whose name has *price* or *cost* in it | `$6.39` |
| a row (from `lowest`, `highest`) | its first three fields: `title Sharp Objects, price £47.82, stock In stock` |
| a list | `12 rows` |
| nothing (an empty value) | `nothing` |

## Steps

### Pages

Every step here needs `website` first.

#### `website <address>`

```text
website walmart.com
website https://books.toscrape.com/catalogue/page-2.html
```

Starts the browser and opens the address. Once per script (use `go to` after that). An
address without `https://` gets it. The browser is Chromium, on the machine's own screen,
with a profile kept on the machine: cookies and sign-ins from yesterday's run are still
there today (see [Machines and browsers](machines.md#the-browser)).

#### `go to <address>`

```text
go to walmart.com/cp/motor-oil/1074301
```

Opens another page in the same browser tab.

#### `type "<text>" into <element>`

```text
type "0W-20 motor oil" into search box
type "{EMAIL}" into email
```

Clicks the element, clears it, and types the text a key at a time, the way a person would.
Waits up to 30 seconds for the element to show.

#### `click <element>`

Clicks it, then gives the page up to 15 seconds to finish loading. Waits up to 30 seconds for
the element to show first.

#### `press <key>` and `press <key> in <element>`

```text
press enter
press escape
press page down
press enter in search box
```

Keys by name: `enter` (or `return`), `tab`, `escape` (or `esc`), `space`, `backspace`,
`delete`, `up`, `down`, `left`, `right`, `page up`, `page down`. Any other word goes to the
browser as a key name, so `press F5` and `press Control+A` work. Without `in`, the key goes
to whatever has focus.

#### `wait for <element or list>`

Waits until it shows, up to 30 seconds, and fails the run if it never does. Use it after a
step that changes the page (a search, signing in) so the next step finds what it needs.

#### `wait <n> seconds`

```text
wait 3 seconds
wait 0.5 seconds
```

A pause, up to 600 seconds. `wait for` is nearly always better: it goes on the moment the
page is ready.

#### `scroll down` and `scroll to the bottom`

`scroll down` scrolls once, a screen or so. `scroll to the bottom` keeps scrolling until the
page stops growing (or 60 scrolls), for sites whose lists load more as you scroll. Do it
before `read`.

### Reading

#### `read <list>`

```text
read listings
```

Reads every item of the list on the page now into rows, one per item, with a value for each
field. Says in the log how many it found. Reading a list again (after `go to`, say) replaces
its rows, and fields worked out from them (`measure`, `divide`) have to be worked out again.

A list must be read before any step that uses its rows.

#### `read <element> as <name>`

```text
element stock note = .availability
read stock note as stock
```

Reads one element's text into a value. The value's name cannot be the element's own name,
so `read price as price` is refused: name the element `price tag`, or the value `price now`.

### Changing rows

These work on rows already read. Each makes the list's rows anew; nothing is changed on the
page.

#### `keep <list> where <field> <test>` and `drop <list> where <field> <test>`

```text
keep listings where title has "0W-20"
keep listings where title matches "0W-?20"
drop listings where price is empty
keep listings where quarts is not empty
keep listings where price is below 40
drop listings where rating is above 4.5
```

`keep` keeps the rows that pass; `drop` throws them out.

| Test | Passes when the field |
|---|---|
| `has "text"` | contains the text, case aside |
| `matches "pattern"` | matches a [regular expression](https://docs.python.org/3/library/re.html#regular-expression-syntax), case aside. `0W-?20` matches 0W-20 and 0W20. |
| `is empty` | has no value, or only spaces |
| `is not empty` | has a value |
| `is below <n>` (or `is under`) | holds a number smaller than n |
| `is above <n>` (or `is over`) | holds a number bigger than n |

`is below` and `is above` read the [number in the text](#numbers-in-text), so
`price is below 40` works on "$31.97". A row with no number fails both.

#### `keep <list> unique by <field>`

```text
keep listings unique by id
```

Keeps the first row of each value of the field. Sites often show the same item twice (a
sponsored copy, a "best seller" strip); this counts it once. Rows where the field is empty
are all kept.

#### `measure <list> <field> in <unit> as <name>`

```text
measure listings title in quarts as quarts
measure listings size in ounces as ounces
```

Reads an amount out of the field's text and gives each row a new field with it, in the unit
you name: "Mobil 1 0W-20, 5 Quart" gives 5; "6 Pack 1 qt" gives 6; "1 Gallon" gives about 4.
Rows that do not say get an empty field. The units and the spellings it reads are under
[Amounts and units](#amounts-and-units).

#### `divide <list> <field> by <field or number> as <name>` and `multiply ...`

```text
divide listings price by quarts as price per quart
multiply listings price by 1.0825 as price with tax
divide listings cents by 100 as dollars
```

A new field for every row. Either side can be text holding a number. When a number is
missing (or you would divide by zero), the new field is empty for that row. Results keep four
decimals.

#### `sort <list> by <field> [lowest first | highest first]`

```text
sort listings by price per quart lowest first
sort reviews by stars highest first
```

`lowest first` is the default (`ascending` and `descending` also work). Rows with no number
in the field go last, in the order they were.

### Summing up

#### `average`, `sum`, `min`, `max`, `median`: `<list> <field> as <name>`

```text
average listings price per quart as average price per quart
median listings price as typical price
sum cart price as total cost
min listings price as cheapest price
```

A number from the field across all rows. Rows without a number are skipped. When no row has
one, the value is empty (`sum` gives 0).

#### `lowest` and `highest`: `<list> <field> as <name>`

```text
lowest listings price per quart as best deal
highest listings rating as best rated
```

The whole row with the lowest (or highest) number in the field, so you keep its title, link
and the rest. Post it, or put it in text (its first three fields are written).

#### `count <list> as <name>`

How many rows the list has now.

### Results

#### `post <list or value> to castle [as "<dataset>"]`

```text
post listings to castle
post best deal to castle
post average price to castle as "average today"
```

Saves into Castle's database, in a **dataset**: the list's or value's own name, or the name
you give in quotes. A list posts all its rows, a row posts one, a plain value posts one row
with a `value` column. Each post of a dataset replaces what Clones shows for it, and every
post stays in the database with its run. Up to 2000 rows a post.

#### `track <value> [lower is better | higher is better]`

```text
track average price per quart lower is better
track listings found higher is better
track average price
```

Keeps a number day after day: the numbers in Clones' chart and in the update after each run.
A day counts once, with its last reading. Saying which way is better lets the update
say "Buy today", "Good day to buy", "Wait if you can"; without it, the update only says how
today compares with the average. A value can be tracked once per script. If the value holds
no number on a run, the log says so and nothing is kept that day.

[Results and the daily update](results.md) has the exact wording rules.

### The log and pictures

#### `say "<text>"`

```text
say "{listings found} listings, {average price per quart} a quart on average"
```

A line in the run's log (**What the script said** in Clones). The last thing a script said
is shown on its run when it finishes.

#### `screenshot ["<label>"]`

```text
screenshot
screenshot "the search results"
```

A picture of the page, kept with the run and shown on its card. Up to 30 a run. A failing
step takes one by itself.

### Checks and guards

#### `fail if <list or value> is empty ["<message>"]`

```text
fail if listings is empty "Walmart showed no listings: see the screenshot."
fail if page title is empty
```

Ends the run, as failed, with the message (or `<name> is empty`). Put one after `read` and
after every `keep`: a run that ends saying *why* beats one that posts nothing.

#### `fail if <element> is shown ["<message>"]`

```text
element no results = text "We couldn't find"
fail if no results is shown "The search found nothing."
```

Looks for the element for up to 2.5 seconds, once. If it shows, the run ends with the message.
If not, the run goes on.

#### `pause whenever <element> is shown ["<message>"]`

```text
element robot check = text "Robot or human?"
pause whenever robot check is shown "Open this bot's screen, take control, and answer it."
```

From this line to the end of the script, before every step and while any step waits: when
the page shows the element, the run waits for a person. Clones shows **waiting for you**
and the message on the run. Open the bot's screen, **take control** (a container) or use the
VM's window, deal with it, and the run goes on by itself once the element is gone. After 10
minutes with nobody, the run fails.

Put it near the top, right after the declarations. This is how Castle deals with robot
checks, sign-in codes and anything else a person has to answer: it never answers them itself.

#### `fail whenever <element> is shown ["<message>"]`

The same watch, but the run ends at once with the message.

```text
element error page = text "Something went wrong"
fail whenever error page is shown "The site is having trouble."
```

`visible` works in place of `shown` in all three.

### Python

#### `python:`

```text
python:
    rows = v["listings"]
    run.say("the first title is " + (rows[0]["title"] or "?"))
```

The indented lines under `python:` run as Python, as one step. Use it for what the language
cannot say yet; if you find yourself using the same block twice, that is a step the language
should have.

What is there:

| Name | What it is |
|---|---|
| `v` | the script's values and lists, by name in lower case: `v["listings"]` is a list of dicts, `v["average price"]` a number |
| `run` | the run: `run.say(text)`, `run.post(dataset, rows)`, `run.track(name, number, "lower")`, `run.screenshot(label)`, `run.text("{name}")` |
| `run.web.page` | the browser's [Playwright page](https://playwright.dev/python/docs/api/class-page), after `website` |
| `castle` | the library: `castle.number(text)`, `castle.data.amount_in(text, "quart")`, and the functions the steps call (`castle.keep`, `castle.average`, ...) |

Raise an exception to fail the step: the run ends there with your message and a picture,
like any step. `raise castle.StepFailed("why")` gives a clean message.

The compiler does not read the Python, so a value made inside it is not a name the script
knows: `{x}` in a later `say` is refused. Put it in text from Python instead
(`run.say(...)`), or post it from there.

## Numbers in text

Pages write numbers as text: "$31.97", "Now $5.48 /qt", "1,299 reviews". Every step that needs
a number (`keep ... is below`, `divide`, `sort`, `average`, `track`...) finds it like this:

1. A **currency amount** first: the first `$`, `€` or `£` followed by a number. "Was 3 for
   $10" gives 10.
2. Otherwise **the first number** in the text. "4.5 out of 5 stars" gives 4.5.
3. Commas between digits are dropped: "1,299.00" is 1299.
4. No number at all: empty. Rows with empty numbers are skipped by sums and averages, fail
   `is below`/`is above`, and go last in `sort`.

So pick the selector that holds only the number you mean. A price box that says "Was $9.99
Now $7.99" gives 9.99, the first currency amount. Point the field at the "now" price instead.

## Amounts and units

`measure` reads how much an item holds, from text like a product title.

| You write | Reads | Same kind as |
|---|---|---|
| `quarts` (`quart`, `qt`) | qt, qts, quart, quarts | volume |
| `gallons` (`gallon`) | gal, gallon, gallons | volume |
| `liters` (`liter`, `litre`, `litres`) | l, ltr, liter, litre | volume |
| `milliliters` | ml, milliliter, millilitre | volume |
| `fluid ounces` (`fl oz`) | fl oz, fluid ounce; plain oz too, for volume | volume |
| `pounds` (`pound`) | lb, lbs, pound | weight |
| `ounces` | oz, ounce | weight |
| `grams` | g, gram | weight |
| `kilograms` | kg, kilogram | weight |
| `items` | count, ct, piece, pcs | count |

- It converts within a kind: `in quarts` reads "1 Gallon" as 4 and "946 ml" as about 1. It never
  converts between kinds (a weight is not a volume).
- The **first** amount of the right kind in the text is the one used.
- **Packs** multiply: "6 Pack", "6-pk", "Pack of 6" and "6 x 1 qt" make the amount six
  times as much (for packs of 2 to 100).
- "oz" on a liquid is fluid ounces: `measure ... in quarts` reads "32 oz" as 1 quart.

## When a step fails

A step fails when what it needs never shows (30 seconds), the page refuses a click, a check
says so, or its Python raises. Then:

1. Castle takes a picture of the page, labelled "line N failed".
2. The run ends as **FAILED**, with the line, the step as you wrote it, and a message.
   Timeouts say "the page did not show it in time".
3. The browser closes and, unless the automation runs again and again, the machine stops.

Everything posted and tracked before the failure stays.

If the machine could not get ready (no Python, Playwright would not install), the run fails
on line 0, "getting the machine ready", with the reason.

## What the compiler says

The problems you are most likely to see, and what to do:

| It says | Do |
|---|---|
| `Castle does not know "clik"` | A typo in the first word. The message lists the steps. |
| `"search box" is not an element. Declare it first: element search box = <selector>` | Add the `element` line above, or fix the name. |
| `read listings first` | Add `read listings` before the step. |
| `listings has no field "cost". It has: title, price, link` | Use one of the fields it lists. |
| `say which website first: website example.com` | A page step comes before `website`. |
| `the website is already set on line 3. Use "go to" to open another page` | One `website` per script. |
| `{total} is not a value yet...` | Make the value earlier with `... as total`, or write a bot field in capitals: `{TOTAL}`. |
| `"price as text" cannot be a name...` | A name with `as` or `where` in it. Pick another. |
| `a quote is opened and never closed` | Close the quoted text. |
| `only a list's fields are indented` | Take the spaces off the front, or put the line under a `list`. |
| `price is already an element or a list` | `read price as price`: give the value another name. |
| `measure knows quarts, gallons, ...` | Use one of the units listed. |
| `end it with: as <a name for the answer>` | The step makes a value: name it. |
| `the script uses {EMAIL}: add a field called EMAIL under What each bot fills in` | (When you start it.) Add the field to the automation. |
| `this bot has no email yet` | (When you start it.) Fill in the bot's field. |

## What a script becomes

Castle turns the script into one Python file, `main.py`. Each step becomes a block that
carries its line, which is how Clones knows the line a run is on and the line it failed at.
From the Walmart example, shortened:

```python
import castle

run = castle.start()
v = run.values
ELEMENTS = {"search box": "input[name=\"q\"]", "search button": "button[aria-label=\"Search\"]"}
LISTS = {"listings": {"selector": "[data-item-id]", "fields": [["title", "[data-automation-id=\"product-title\"]", ""]]}}

with run.step(12, "website walmart.com"):
    run.website("https://walmart.com")
with run.step(24, "type \"0W-20 motor oil\" into search box"):
    run.web.type(ELEMENTS["search box"], run.text("0W-20 motor oil"))
with run.step(28, "read listings"):
    v["listings"] = run.web.read_list(LISTS["listings"]["selector"], LISTS["listings"]["fields"])

run.finish()
```

**The Python it becomes** under the editor shows yours. Castle writes it fresh from the
script on every start, so change the script, not the Python.

## Limits

| What | Limit |
|---|---|
| A script | 64 KB |
| A line | 2000 characters |
| A name | 60 characters |
| `wait <n> seconds` | 600 seconds |
| `wait for`, and any step waiting for its element | 30 seconds |
| `fail if <element> is shown` | looks for 2.5 seconds |
| `pause whenever`: a person has | 10 minutes |
| A `post` | 2000 rows |
| Rows a run keeps | 20,000 |
| Pictures a run keeps | 30, up to 6 MB each |
| Log lines a run keeps | 2000 |
| Clones shows | the last 30 runs, 300 rows a dataset, 180 days of a tracked value |
