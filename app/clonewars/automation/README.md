# Castle scripts

An automation in Castle can be written as a **Castle script**: plain lines, one step each,
written in the Clones screen. Castle compiles the script into a Python project, brings the
`castle` library with it, runs it in a container or a VM, and keeps what it finds in its
database. Nothing has to exist outside Castle.

```text
castle 1
website walmart.com

element search box    = input[name="q"]
element search button = button[aria-label="Search"]
list listings         = [data-item-id]
  field title = [data-automation-id="product-title"]
  field price = [data-testid="unified-global-product-price-a11y-text"]

type "0W-20 motor oil" into search box
click search button
wait for listings
read listings
keep listings where title matches "0W-?20"
measure listings title in quarts as quarts
divide listings price by quarts as price per quart
average listings price per quart as average price per quart
post listings to castle
track average price per quart lower is better
```

The whole example is [`examples/walmart-motor-oil.castle`](examples/walmart-motor-oil.castle),
which every box has as the **Walmart Motor Oil Prices** automation.

## How it runs

| Piece | Where | What it does |
|---|---|---|
| Compiler | `castled/src/script.rs` | Checks the script as a whole (every problem, with its line) and writes `main.py`: one `with run.step(line, ...)` block per step. |
| Library | `castle/` (this folder) | What `main.py` calls. `__init__.py` is the run and its reports, `data.py` what is done to rows, `web.py` the browser (Playwright, driving Chromium on the machine's own screen). |
| Runner | `castle/run.sh` | Makes `~/.castle/venv` and installs Playwright the first time, then runs `main.py`. |
| Results | `castled/src/results.rs` | Runs, rows, tracked values, log lines and pictures in `results.db`; the update in words after each run. |

On every start Castle writes the project fresh (`data/clonewars/automations/<automation>-<bot>/`):
`automation.castle`, `main.py`, this `castle/` folder, and `castle-run.json`, the run's
ticket: the addresses of the box as the machine sees it and a secret that lets this run,
and only this run, report. The machine gets Python and Chromium from its packages
(`python3-venv chromium fonts-liberation`; `python chromium ttf-liberation` on Arch). The
script runs as the machine's desktop user, on its screen, so the screen view in Clones shows
the browser and **Take control** reaches it. The browser keeps its profile in
`~/.castle/browser`, so a site sees the same browser come back day after day.

When the script reaches its end, the box stops the machine, unless the automation runs again
and again. A step that fails ends the run with its line, a message and a picture of the page.

Containers are the light choice (Debian is the one with a working Chromium). A VM works the
same way; a temporary one starts clean every time and installs the browser each run.

## The steps

Names (`search box`, `price per quart`) are declared before they are used, may have spaces,
and are matched longest first. `as` and `where` cannot be names. Lines starting with `#` are
comments.

| Step | Does |
|---|---|
| `castle 1` | The language version. Optional; if it is there, it is the first line. |
| `website walmart.com` | Opens the browser on a site. Web steps need it first. Once per script. |
| `element <name> = <selector>` | Names something on the page. |
| `list <name> = <selector>` | Names a repeating thing; its fields are indented under it. |
| `  field <name> = <selector>` | One field of each item: the text of the first match inside it. `a @href` reads an attribute; `@data-id` reads one of the item itself. |
| `go to <address>` | Opens another page. |
| `type "<text>" into <element>` | Types, a key at a time. |
| `click <element>` | Clicks. |
| `press <key> [in <element>]` | enter, tab, escape, space, backspace, delete, up, down, left, right, page up, page down, or a browser key name. |
| `wait for <element or list>` | Until it shows, up to 30 seconds. |
| `wait <n> seconds` | Up to 600. |
| `scroll down` / `scroll to the bottom` | The second scrolls until the page stops growing, for listings that load on scroll. |
| `read <list>` | Every item into rows. |
| `read <element> as <name>` | One element's text into a value. |
| `keep <list> where <field> <test>` | `has "text"`, `matches "pattern"` (case does not matter), `is empty`, `is not empty`, `is below 10`, `is above 10`. `drop` keeps the others. |
| `keep <list> unique by <field>` | The first of each. |
| `measure <list> <field> in <unit> as <name>` | The amount a text says: quarts, gallons, liters, milliliters, fluid ounces, pounds, ounces, grams, kilograms, items. "6 Pack 1 qt" is 6 quarts, "1 Gallon" is 4. |
| `divide <list> <field> by <field or number> as <name>` | A new field. `multiply` too. |
| `sort <list> by <field> [lowest first or highest first]` | Rows without a number go last. |
| `average`, `sum`, `min`, `max`, `median` `<list> <field> as <name>` | A number. Text holding a number ("Price $31.97") counts as that number. |
| `lowest`, `highest` `<list> <field> as <name>` | The whole row. |
| `count <list> as <name>` | How many rows. |
| `post <list or value> to castle [as "<dataset>"]` | Into Castle's database. The newest post of each dataset is what Clones shows. |
| `track <value> [lower is better or higher is better]` | Kept day after day. After each run Castle writes how today compares: the lowest in weeks, a good day, a high day, about usual, and after two weeks the weekday that is usually best. |
| `say "<text>"` | A line in the run's log. |
| `screenshot ["<label>"]` | A picture of the page, kept with the run. |
| `fail if <list or value> is empty ["<message>"]` | Ends the run. |
| `fail if <element> is shown ["<message>"]` | Ends the run when the page shows something the script cannot go on from (looks for a moment). |
| `pause whenever <element> is shown ["<message>"]` | From here on, before every step and while waiting: when the page shows it, the run waits up to ten minutes for a person to deal with it (Clones shows the message on the run), then goes on. |
| `fail whenever <element> is shown ["<message>"]` | The same watch, but the run ends. |
| `python:` | Indented Python under it, for what the language cannot say yet. `run`, `v` (the values) and `castle` are there. |

In quoted text, `{name}` is a value of the script and `{NAME}` (capitals) a sign-in field of
the bot: add the field under *What each bot fills in*, and each bot keeps its own.

Selectors are the browser's own: CSS, or Playwright's `text=` and `xpath=`. Four plain forms
also work: `text "Search"`, `label "Search"`, `placeholder "Search Walmart"`,
`title "Search"`.

## Sites that ask whether you are a robot

Some sites, Walmart among them, sometimes ask a browser to prove it is a person. Castle does
not try to get around that. The Walmart example watches for the question
(`pause whenever robot check is shown`): the run waits, and Clones shows "waiting for you"
on it. Open the bot's screen, take control (a container) or use the VM's own window, and
answer it; the run goes on by itself. The browser profile is kept on the machine's disk, so
the runs after it usually get through. A temporary machine forgets it every run.

## Trying a script outside a machine

`CASTLE_HEADLESS=1` runs the browser without a window and `CASTLE_BROWSER_PROFILE` points
it at another profile folder, for running a compiled `main.py` by hand.
