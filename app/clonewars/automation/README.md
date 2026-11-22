# Castle scripts

**Castle script** is Castle's own language for automations. You write what a person would
do on a website, one plain line per step. Castle checks it, turns it into a Python
project, runs it in a container or a VM with a real browser on the machine's screen, and keeps
what it finds in its database, day after day. You write it in the Clones screen, and nothing
has to exist outside Castle.

```castle
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

That is most of the **Walmart Motor Oil Prices** automation every box comes with. Each day
it tells you whether today is a good day to buy:

> Average price per quart — good day to buy: $6.71, 3% under the 20-day average of $6.93.

## Read on

These pages are also in the app, under **Clones → Castle script**, where every whole script
has a **Try this script** button that starts a new automation from it.

| | |
|---|---|
| **[Your first script](docs/tutorial.md)** | Start here. Two small scripts on practice sites: read a list, work out a number, keep it, sign in, run every day. |
| **[The language](docs/language.md)** | Every step, every rule, every limit, and what the compiler's messages mean. |
| **[Cookbook](docs/cookbook.md)** | Finding selectors, searching, price per unit, more than one page, pop-ups, robot checks, debugging a failed run. |
| **[Machines and browsers](docs/machines.md)** | Container or VM, what gets installed, the browser, Firefox and Chrome, Dockerfiles, how runs reach the box. |
| **[Results and the daily update](docs/results.md)** | What is kept, how the update in words is worked out, daily runs, the database. |
| **[Examples](examples/)** | Whole scripts: [Walmart motor oil](examples/walmart-motor-oil.castle), [bookshop prices](examples/books-mystery-prices.castle), [signing in](examples/quotes-sign-in.castle). |

## Writing one in five lines

1. In **Clones**, press **New automation** and pick **Castle script**. (Or **Walmart
   example** to start from a copy of the Walmart script.)
2. `website` and the address.
3. Right-click what you want on the site, **Inspect**, and name it: `element` for one
   thing, `list` with indented `field` lines for something that repeats.
4. Say what to do with them: `type`, `click`, `read`, `keep`, `average`, `post`, `track`.
5. **Create automation**, then **Start**. The card shows the line it is on, then the
   results.

The editor checks the script as you type and lists every problem with its line. **Steps**
shows the steps; **The Python it becomes** shows what Castle will run.

## The rules in one breath

One step per line, top to bottom, no loops. `#` starts a comment line. Names are plain words,
may have spaces, and are declared before they are used; the longest name wins, and `as` and
`where` cannot be in one. Only a list's fields (and Python under `python:`) are indented.
Quoted text can carry `{a value}` or a bot's `{SIGN_IN_FIELD}`. Selectors are the browser's
own, plus `text "..."`, `label "..."`, `placeholder "..."` and `title "..."`. Every step
waits up to 30 seconds for what it needs; a step that fails ends the run with its line, a
message and a picture. Robot checks are for people: `pause whenever` waits for one to answer.

## How it runs

| Piece | Where | What it does |
|---|---|---|
| Compiler | [`castled/src/script.rs`](../../../castled/src/script.rs) | Checks the whole script (every problem, with its line) and writes `main.py`: one `with run.step(line, ...)` block per step. |
| Library | [`castle/`](castle) | What `main.py` calls. `__init__.py` is the run and its reports to the box, [`data.py`](castle/data.py) what is done to rows, [`web.py`](castle/web.py) the browser (Playwright driving Chromium). |
| Runner | [`castle/run.sh`](castle/run.sh) | Makes `~/.castle/venv` and installs Playwright the first time, then runs `main.py`. |
| Results | [`castled/src/results.rs`](../../../castled/src/results.rs) | Runs, rows, tracked values, log lines and pictures in `results.db`; the update in words after each run. |
| Editor | [`ScriptEditor.svelte`](../../src/components/clones/ScriptEditor.svelte), [`ResultsPanel.svelte`](../../src/components/clones/ResultsPanel.svelte) | Writing a script in Clones, and its results on the automation's card. |

On every start Castle writes the project fresh (`app/data/clonewars/automations/<automation>-<bot>/`):
`automation.castle`, `main.py`, the `castle/` library and `castle-run.json`, the run's ticket.
The machine installs Python and Chromium from its packages, runs the script as its desktop
user on its screen, and reports every step to the box as it goes. When the script reaches
its end, or fails, the box stops the machine, unless the automation runs again and again.

## Trying a script outside a machine

`CASTLE_HEADLESS=1` runs the browser without a window and `CASTLE_BROWSER_PROFILE` points it
at another profile folder, for running a compiled `main.py` by hand with Playwright
installed. Without a `castle-run.json` next to it, the script runs without reporting and its
reports go to `~/castle-results.ndjson`.

## Changing the language

The compiler and this documentation move together. Every `castle` code block in these pages
and every file in `examples/` is compiled by the compiler's tests
(`cargo test -p castled script`), so a page that shows a script the compiler would refuse
fails the build. Fragments that are not whole scripts are fenced as `text`.
