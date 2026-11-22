# Castle script cookbook

Short recipes for the things scripts usually need. Each one is a few lines to drop into a
script. The steps are explained in full in [the language](language.md).

- [Finding a selector](#finding-a-selector)
- [Search a site](#search-a-site)
- [Price per quart, per ounce, per item](#price-per-quart-per-ounce-per-item)
- [Only what is in stock, or under a price](#only-what-is-in-stock-or-under-a-price)
- [The cheapest one](#the-cheapest-one)
- [One price off one product page](#one-price-off-one-product-page)
- [Sign in with the bot's own account](#sign-in-with-the-bots-own-account)
- [More than one page](#more-than-one-page)
- [Lists that load as you scroll](#lists-that-load-as-you-scroll)
- [A pop-up that only sometimes shows](#a-pop-up-that-only-sometimes-shows)
- [Robot checks and codes a person has to answer](#robot-checks-and-codes-a-person-has-to-answer)
- [Pages that mean "stop"](#pages-that-mean-stop)
- [A daily update](#a-daily-update)
- [When a run fails](#when-a-run-fails)

## Finding a selector

1. Open the site in Chrome, Edge or Firefox, right-click the thing you want, and pick
   **Inspect**. The developer tools open with its HTML picked out.
2. Look for something that names it and will not change: an `id`, a `name`, an
   `aria-label`, or a `data-...` attribute. Those are written by people and outlast a
   redesign. Class names like `css-1x2y3z` or `sc-kpOJdX` are made by a build tool and change
   whenever the site ships.
3. Try it before you write it down: in the developer tools' **Console**, type
   `$$('your selector').length`. For an element you want 1 (or the first to be the right one);
   for a list, the number of items you see.
4. For a list, find the box that holds one item (one product, one row), the smallest box that
   has everything you want from it. Then each field is a selector *inside* that box.

Good choices, best first:

| Prefer | Like |
|---|---|
| a test or data attribute | `[data-testid="price"]`, `[data-item-id]`, `[data-automation-id="product-title"]` |
| a form field's name or id | `input[name="q"]`, `#email` |
| an accessible label | `button[aria-label="Search"]` |
| the words it shows | `text "Add to cart"`, `label "Email"`, `placeholder "Search"` |
| a short path | `h3 a`, `.product .price` |

**Copy selector** in the developer tools' right-click menu gives something that works today,
but is usually a long path like `#main > div:nth-child(3) > ...` that breaks with the next
change. Use it to find your way, then write a short one.

**Check what the page really has.** Some sites build the page in the browser after it
loads, and some show different things to different browsers. If a selector works in your
browser but the run finds nothing, take a `screenshot` just before the step and look at what
the machine's browser saw.

## Search a site

```text
element search box    = input[name="q"]
element search button = button[aria-label="Search"]
list results          = [data-item-id]
  field title = [data-automation-id="product-title"]
  field price = [data-testid="price"]

type "0W-20 motor oil" into search box
click search button
wait for results
read results
fail if results is empty "The search showed nothing: see the picture."
```

No search button you can name? `press enter in search box` instead of `click`.

Going straight to the results address is often simpler and faster:

```text
website walmart.com/search?q=0W-20+motor+oil
```

## Price per quart, per ounce, per item

```text
measure results title in quarts as quarts
keep results where quarts is not empty
divide results price by quarts as price per quart
sort results by price per quart lowest first
average results price per quart as average price per quart
```

`measure` reads "5 Quart", "6 Pack 1 qt" or "1 Gallon" out of the title. Drop the rows it
could not read, or their empty value would leave them out of the averages anyway, but still
in your table. Use `ounces`, `pounds`, `liters`, `items` the same way; the full list is in
[Amounts and units](language.md#amounts-and-units).

## Only what is in stock, or under a price

```text
keep results where stock has "In stock"
drop results where title has "refurbished"
keep results where price is below 40
keep results where title matches "0W-?20"
```

`has` is plain words, case aside. `matches` is a pattern: `0W-?20` is 0W-20 or 0W20,
`\b(5|6) ?qt` is 5 qt or 6qt.

## The cheapest one

```text
lowest results price per quart as best deal
post best deal to castle
say "Best deal: {best deal}"
```

`lowest` keeps the whole row: title, price, link. Posting it makes a one-row table you can
open from Clones.

## One price off one product page

```castle
castle 1
website example-shop.com/product/12345

element price tag = [data-testid="price"]
read price tag as price now
fail if price now is empty "The product page showed no price."
post price now to castle
track price now lower is better
```

The value cannot be called what the element is, so the element is `price tag` and the value
`price now`. A name with "price" in it is written with `$` in the update and the log.

## Sign in with the bot's own account

```text
element email    = input[name="email"]
element password = input[type="password"]
element sign in  = button[type="submit"]
element account  = text "My account"

type "{SHOP_EMAIL}" into email
type "{SHOP_PASSWORD}" into password
click sign in
wait for account
```

Add `SHOP_EMAIL` and `SHOP_PASSWORD` under *What each bot fills in* (mark the password
**Secret**), and fill them in on each bot. Capitals in braces are always bot fields; Castle
refuses to start a script whose fields the automation does not have.

The browser keeps its cookies on the machine, so a site usually remembers it is signed in. If
the sign-in form is not there the next time, `type` waits for it and fails. Two ways round
that: run on a [temporary machine](machines.md#kept-or-temporary), which forgets every time,
or go straight to a page behind the sign-in and let a guard catch the form:

```text
element sign-in form = input[type="password"]
pause whenever sign-in form is shown "Sign this bot in, then the run goes on."
```

A code sent by text or e-mail is the same: `pause whenever` it is asked for, and answer it
through the bot's screen.

## More than one page

A script has no loops. Read each page in turn and post each under its own name:

```text
read results
post results to castle as "page 1"
go to shop.example.com/search?q=oil&page=2
read results
post results to castle as "page 2"
```

Or let Python gather them into one list (each `read` replaces the rows, so collect them as you
go):

```text
read results
python:
    every = list(v["results"])
    for n in range(2, 6):
        run.web.go(f"https://shop.example.com/search?q=oil&page={n}")
        every += run.web.read_list(LISTS["results"]["selector"], LISTS["results"]["fields"])
    v["results"] = every
    run.say(f"{len(every)} results over 5 pages")
keep results unique by link
average results price as average price
```

The steps after the block see all the rows, because the block put them back in `v["results"]`.

## Lists that load as you scroll

```text
wait for results
scroll to the bottom
read results
```

`scroll to the bottom` scrolls until the page stops getting longer. For a list that shows
the first few and a **Show more** button, name the button and click it, then
`wait for results` again.

## A pop-up that only sometimes shows

`click` waits for its element and fails the run when it never comes, so a cookie banner that
shows only on the first visit cannot be a plain `click`. A guard waits for a person; for a
banner you want clicked away, Python:

```text
python:
    banner = run.web.page.locator("#onetrust-accept-btn-handler")
    if banner.count() and banner.first.is_visible():
        banner.first.click()
```

After the first run the browser remembers the answer, so the banner usually stops showing.

## Robot checks and codes a person has to answer

```text
element robot check = text "Robot or human?"
pause whenever robot check is shown "The site asks whether this is a robot. Open this bot's screen, take control, and answer it."
```

Put it right after the declarations. From that line on, whenever the check shows, the run
waits up to 10 minutes and Clones shows **waiting for you** with your message on the run.
Open the bot's screen, **take control**, answer it, and the run goes on by itself.

Castle never answers these itself and has no way to slip past them: a site that asks
whether you are a person is asking for a person. What helps is what helps any browser:

- **Keep the machine** (not temporary), so the browser's profile and the site's memory of
  it carry over from run to run.
- **Run at a sensible pace**: once a day is a visitor; every minute is not.
- **Go straight to the page** you need rather than through the home page and a search.

## Pages that mean "stop"

```text
element no results = text "We couldn't find"
element error page = text "Something went wrong"

fail whenever error page is shown "The site is having trouble today."
type "0W-20 motor oil" into search box
press enter in search box
fail if no results is shown "The search found nothing today."
```

`fail if ... is shown` looks once, briefly. `fail whenever` watches from its line to the end.

## A daily update

1. `track` the number that matters, saying which way is better:
   ```text
   average results price per quart as average price per quart
   track average price per quart lower is better
   ```
2. Press **Change** on the automation and tick **Run every day at**.

After each run the card's first line is the update: "Buy today", "Good day to buy",
"About usual", "Wait if you can", and after two weeks, the weekday that has usually been
best. [Results and the daily update](results.md) says exactly when it says what.

## When a run fails

Clones shows the line it failed at, the step as you wrote it, a message and a picture.

| The message | Usually |
|---|---|
| `the page did not show it in time` | The selector finds nothing on that page. Look at the picture: a different page (a robot check, a sign-in, a pop-up), or the site changed its HTML. Check the selector in your own browser. |
| your own `fail if` message | The check did its job; the picture shows why. |
| `<name> is empty` | A `read` or `keep` left no rows. Loosen the `keep`, or fix the field selectors. |
| `{EMAIL} has no value` | The bot has no value for that field. |
| `getting the machine ready` (line 0) | The machine could not install Python or Playwright, usually because it is offline. |
| `the script stopped with a Python error` | `main.py` stopped outside any step, before the first one or after the last. Rare; the machine's own log, `~/castle-automation.log`, has the Python error. |

More to look at:

- **What the script said** on the card: every line the run logged, with its line number.
- The bot's **screen**, live, while it runs. Take control to try things by hand in the same
  browser the script uses.
- **The Python it becomes**, under the editor.
- A run that never reports at all usually could not reach the box: see
  [How a run reaches the box](machines.md#how-a-run-reaches-the-box).
