# Your first Castle script

This walks through two small scripts on sites made for practice, so nothing here can get
anyone in trouble and the pages stay the same. By the end you will have read a list off a
page, worked a number out of it, kept it in Castle's database, signed in with a bot's own
username and password, and set the script to run every day.

You need a Castle box with Clones turned on and Docker Desktop running (or a Linux VM, if you
prefer; see [Machines and browsers](machines.md)).

## Part 1: prices in a bookshop

[books.toscrape.com](https://books.toscrape.com) is a pretend bookshop. We will read the
mystery shelf, average the prices, and keep the average day after day.

### 1. Make the automation

In **Clones**, press **New automation**. Under *What it runs* pick **Castle script**, give it
a name ("Mystery book prices"), and leave *Runs in* on the container with the **Debian 13
container** system. The editor starts you with a short script to replace.

### 2. Say where to go

```text
castle 1
website books.toscrape.com/catalogue/category/books/mystery_3/index.html
```

`castle 1` is the language version. It is optional, but it keeps a script readable by future
Castles. `website` opens the browser on a page. You can leave off `https://`.

### 3. Name what you want from the page

Open the page in your own browser, right-click the first book and pick **Inspect**. Each book
is an `<article class="product_pod">`. Inside it, the title is in the `title` of the link in
the `<h3>`, and the price is in `<p class="price_color">`.

A **list** is something that repeats on the page. Its **fields** are what you want from
each one, indented under it:

```text
list books = article.product_pod
  field title = h3 a @title
  field price = .price_color
  field stock = .availability
  field link  = h3 a @href
```

`h3 a @title` means "the `title` attribute of the link in the h3". Without `@` a field is
the text you see. `@href` links come back as full addresses.

### 4. Read it, and stop if nothing came back

```text
read books
fail if books is empty "The mystery shelf showed no books."
```

`read books` makes one row per book. If the site changes and the list finds nothing, the
`fail if` step ends the run there with your message, a picture of the page and the line
number, instead of carrying on with nothing.

### 5. Work something out and keep it

```text
average books price as average in pounds
post books to castle
track average in pounds lower is better
say "{average in pounds} on average across the mystery shelf"
```

- `average books price` finds the number in each price ("£47.82" counts as 47.82) and
  averages them. `as average in pounds` names the answer so later steps can use it.
- `post books to castle` saves every row in Castle's database.
- `track` keeps the number day after day. `lower is better` tells Castle which way is good,
  so the update after each run can say "Good day to buy" or "Wait if you can".
- `say` writes a line to the run's log. `{average in pounds}` is filled in with the value.

> A value whose name has *price* or *cost* in it is written with a `$` sign. These books
> are priced in pounds, so the name here leaves "price" out.

### The whole script

```castle
castle 1
# Mystery book prices: the tutorial's first script.
website books.toscrape.com/catalogue/category/books/mystery_3/index.html

list books = article.product_pod
  field title = h3 a @title
  field price = .price_color
  field stock = .availability
  field link  = h3 a @href

read books
fail if books is empty "The mystery shelf showed no books."

average books price as average in pounds
post books to castle
track average in pounds lower is better
say "{average in pounds} on average across the mystery shelf"
```

The status above the editor says **READY TO RUN** once the box has checked the script. If it
says **1 PROBLEM**, the list under the editor names the line and what is wrong. Click it to
jump there.

### 6. Run it

Press **Create automation**, then **Start** on the automation's card. The first run sets up the machine:
it installs Python, Chromium and Playwright, which takes a few minutes. Later runs skip all
that. While it runs, the card shows the line it is on. Open the bot's screen to watch the
browser.

When it finishes, the machine stops by itself and the card shows:

- the update in words ("Average in pounds: 32.79 today. This is the first day Castle
  has, so it cannot say yet whether that is good.")
- the tracked value, with a line chart once there are two days of it
- the **books** table: every row it posted, with links you can open
- **What the script said**: the run's log

### 7. Make it pickier

Steps after `read` change the rows. Add these after `read books`:

```text
keep books where stock has "In stock"
sort books by price lowest first
lowest books price as cheapest book
post cheapest book to castle
```

`keep ... where` drops every row that fails the test. `lowest` gives you the whole row of the
cheapest book, not just its price, so `post cheapest book to castle` saves a one-row table
called *cheapest book*.

## Part 2: signing in, and a second page

[quotes.toscrape.com](https://quotes.toscrape.com) has a login form that takes any username
and password. Each bot of an automation can have its own sign-in, so the username and password
are not written in the script. They are **fields** the bot fills in.

### 1. Add the fields

Press **Change** on the automation and, under **What each bot fills in**, press **Add a field**
twice:

| Field (the name in the script) | Label | Secret |
|---|---|---|
| `QUOTES_USER` | Username | no |
| `QUOTES_PASSWORD` | Password | yes |

Save, then fill both in on the bot's panel and save it (anything works on this site). Start
stays off until every field has a value.

### 2. The script

```castle
castle 1
# Quotes: signs in with the bot's own username and password, reads two pages.
website quotes.toscrape.com/login

element username = #username
element password = #password
element log in   = input[type="submit"]
element log out  = text "Logout"
list quotes = div.quote
  field text   = span.text
  field author = small.author
  field tags   = .tags
  field about  = a @href

type "{QUOTES_USER}" into username
type "{QUOTES_PASSWORD}" into password
click log in
wait for log out

read quotes
count quotes as first page count
post quotes to castle as "page 1"

go to quotes.toscrape.com/page/2/
read quotes
keep quotes where tags has "life"
post quotes to castle as "page 2 about life"

say "Signed in. {first page count} quotes on the first page."
```

What is new:

- `element` names one thing on the page. `text "Logout"` finds it by the words it shows,
  which often outlasts a site's class names.
- `{QUOTES_USER}` in capitals is a bot field. Castle checks, before the run starts, that the
  automation has a field by that name.
- `wait for log out` waits up to 30 seconds for the page after signing in to show the
  Logout link, so the script never reads the page before you are signed in.
- `go to` opens another page in the same browser, still signed in.
- `read quotes` a second time replaces the rows with the new page's. Each page is posted
  under its own name with `as "..."`.

## Part 3: every day

Press **Change** on the automation, tick **Run every day at** under *Rules*, and pick a time. The box starts
the first bot at that time on this computer's clock. If the box was off at that time, it starts
the bot when it comes back on later the same day.

Each run adds a reading to every tracked value. After a few days, the update compares today
with the days before it. After two weeks it also says which weekday has usually been best.
[Results and the daily update](results.md) has the exact rules.

## Where to go next

- [The language](language.md): every step, every rule, every limit.
- [Cookbook](cookbook.md): finding selectors, searching, price per unit, pop-ups, robot checks,
  debugging.
- [Machines and browsers](machines.md): containers or VMs, packages, browsers, Docker.
- [`examples/`](../examples): whole scripts, the Walmart one among them. Both scripts on
  this page are there too.
