# Results and the daily update

What a script finds is kept in Castle's database on the box. This page covers what is kept,
how the update in words is worked out, how daily runs are timed, and how to get at the data
yourself.

## What is kept

Every run of a script, from every bot, whether it was started by hand or by its schedule:

| What | From | Shown in Clones as |
|---|---|---|
| The run | every start | its status (starting, running, done, failed, stopped), when, the line it is on or ended at, and its message |
| Rows | `post` | a table per dataset: the rows of its newest post |
| Tracked values | `track` | the number, a line chart, the low and the high |
| Log lines | `say`, and Castle's own (how many rows a `read` found, a pause) | **What the script said** |
| Pictures | `screenshot`, and one when a step fails | small pictures on the run; click for the full size |
| The update | worked out when a run ends | the first line of the results |

All of it is in one SQLite file, `<vault>/.castle/clones/results.db`, with the pictures beside
it in `shots/`. Deleting an automation deletes its results.

## Datasets

A `post` writes into a dataset: the name of the list or value posted, or the name given with
`as "..."`, in lower case. Clones shows each dataset's newest post. Every post stays in the
database with the run that made it, so last week's rows are still there to compare.

Columns are the fields of the rows, in the order the script made them. Links are shown as
**open**.

## Tracked values and the update

Every `track` step adds a reading of a number. Castle counts **one reading a day**, the last
one of that day on the box's clock, and keeps them for good (Clones charts the last 180 days).

After every run that ends, Castle compares today's reading with the days before it, **over the
last 30 days**, and writes one sentence per tracked value. With `lower is better`:

| When today is | The update says |
|---|---|
| the lowest of those days (or equal) | **Buy today**: $6.58 is the lowest in 20 days, 5% under the 20-day average of $6.93. |
| at least 3% under their average | **Good day to buy**: $6.71, 3% under the 20-day average of $6.93. |
| at least 3% over their average | **Wait if you can**: $7.31 is 5% over the 20-day average of $6.93. It was $6.58 on Nov 14. |
| anywhere between | **About usual**: $6.95, close to the 20-day average of $6.93. |

`higher is better` turns it round: "Best day yet", "Good day", "A low day", "About usual".
Without either, the update says how today compares with the average and nothing more.

Each sentence starts with the value's name: *Average price per quart — buy today: ...*

**The first day** there is nothing to compare with: "This is the first day Castle has, so it
cannot say yet whether that is good. A few more days and it can."

**After 14 days**, it also names the weekday that has been best on average, when there is one
that stands out (at least 2% better than the overall average, from at least two readings
on that weekday): "Over 21 days it has been lowest on Tuesdays."

Values whose name has *price* or *cost* in it are written in dollars.

## Daily runs

**Run every day at** in the automation's rules starts the automation's **first bot** every
day at that time:

- The time is the box's own clock, on this computer.
- Like everything that starts machines, schedules run only while the box listens on this
  computer alone, which is how it normally runs.
- If the box was not running at that time, the bot starts when the box comes back, the same
  day. A day that has passed entirely is skipped, not made up.
- Setting or changing the time after it has passed today makes tomorrow the first run, so
  saving at 10:00 a schedule for 08:00 does not start a run at once.
- Runs it starts are marked **on its schedule** in Clones.

Once a day is the right pace for something like prices. Sites treat a browser that comes
back at the same time every day as a visitor; one that comes back every few minutes, as
something else.

## Getting at the data

**From the box's API** (with the box's token), the same calls Clones makes:

| Call | Gives |
|---|---|
| `GET /api/v1/clones/results?path=<automation id>` | the runs, each dataset's newest rows, the tracked values over 180 days, the newest run's log, and the update |
| `GET /api/v1/clones/results/shot?path=<run>/<n>` | one picture, as PNG |
| `POST /api/v1/clones/script/check` with `{"script": "..."}` | compiles a script without running it: its problems, or the Python it becomes |

**From the file**, read-only, with any SQLite tool. The tables:

| Table | Columns |
|---|---|
| `runs` | `id`, `automation`, `bot`, `started`, `finished`, `status`, `line`, `said`, `message`, `update_text`, `trigger` (`start` or `schedule`) |
| `rows` | `run`, `automation`, `dataset`, `at`, `data` (one row, as JSON) |
| `metrics` | `run`, `automation`, `name`, `at`, `value`, `better` |
| `logs` | `run`, `at`, `line`, `text` |
| `shots` | `run`, `n`, `at`, `label` (the picture is `shots/<run>-<n>.png`) |

Times are milliseconds since 1970 (UTC). For example, every 0W-20 reading so far:

```sql
SELECT date(at / 1000, 'unixepoch', 'localtime') AS day, value
FROM metrics
WHERE automation = 'walmart-motor-oil' AND name = 'average price per quart'
ORDER BY at;
```

Read the file while the box runs, but do not write to it; the box is its only writer.
