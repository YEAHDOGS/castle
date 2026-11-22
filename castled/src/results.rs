//! Results: what automations find, kept in a SQLite database on the box.
//!
//! Every start of a Castle script is a run. The script reports to the box as it goes (see
//! clones.rs, `report`): which line it is on, what it says, the rows it posts, the values it
//! tracks and pictures of its screen. All of it lands here, in `results.db` beside the
//! automations list, and the pictures in `shots/`. A tracked value is kept day after day, and
//! after each run the box writes an update in words: today against the days before it, and
//! whether today is a good day (for a price: a good day to buy).

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};

/// Most lines of log one run keeps.
const MAX_LOG_LINES: i64 = 2000;
/// Most rows one run can post, over all its datasets.
const MAX_ROWS: i64 = 20_000;
/// Most pictures one run keeps.
const MAX_SHOTS: i64 = 30;
pub const MAX_SHOT_BYTES: usize = 6 * 1024 * 1024;
/// Runs the overview lists.
const RUNS_SHOWN: i64 = 30;
/// Rows of each dataset the overview carries.
const ROWS_SHOWN: usize = 300;
/// Days of a tracked value the overview carries.
const DAYS_SHOWN: i64 = 180;
const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// "Good day" needs the value this far (in percent) from its average.
const NOTABLE_PERCENT: f64 = 3.0;
/// A weekday counts as the cheapest one when it is this far (in percent) under the average.
const WEEKDAY_PERCENT: f64 = 2.0;
const WEEKDAY_MIN_DAYS: usize = 14;

pub type ResultsResult<T> = Result<T, String>;

pub struct Results {
    db: Mutex<Connection>,
    shots: PathBuf,
}

/// The run a report belongs to.
#[derive(Clone, Debug)]
pub struct RunRef {
    pub id: String,
    pub automation: String,
    pub bot: String,
    pub finished: bool,
}

fn fail(_: impl std::fmt::Display) -> String {
    "the results database could not be used".into()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn local_day(at_ms: i64) -> NaiveDate {
    Local.timestamp_millis_opt(at_ms).single().map(|t: DateTime<Local>| t.date_naive()).unwrap_or_default()
}

impl Results {
    pub fn open(dir: &Path) -> ResultsResult<Self> {
        std::fs::create_dir_all(dir.join("shots")).map_err(fail)?;
        let conn = Connection::open(dir.join("results.db")).map_err(fail)?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             CREATE TABLE IF NOT EXISTS runs (
               id TEXT PRIMARY KEY, automation TEXT NOT NULL, bot TEXT NOT NULL, token TEXT NOT NULL UNIQUE,
               started INTEGER NOT NULL, finished INTEGER, status TEXT NOT NULL, line INTEGER NOT NULL DEFAULT 0,
               said TEXT NOT NULL DEFAULT '', message TEXT NOT NULL DEFAULT '', update_text TEXT NOT NULL DEFAULT '',
               trigger TEXT NOT NULL DEFAULT 'start');
             CREATE INDEX IF NOT EXISTS runs_by_automation ON runs(automation, started);
             CREATE TABLE IF NOT EXISTS rows (run TEXT NOT NULL, automation TEXT NOT NULL, dataset TEXT NOT NULL, at INTEGER NOT NULL, data TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS rows_by_run ON rows(run, dataset);
             CREATE TABLE IF NOT EXISTS metrics (run TEXT NOT NULL, automation TEXT NOT NULL, name TEXT NOT NULL, at INTEGER NOT NULL, value REAL NOT NULL, better TEXT NOT NULL DEFAULT '');
             CREATE INDEX IF NOT EXISTS metrics_by_automation ON metrics(automation, name, at);
             CREATE TABLE IF NOT EXISTS logs (run TEXT NOT NULL, at INTEGER NOT NULL, line INTEGER NOT NULL, text TEXT NOT NULL);
             CREATE INDEX IF NOT EXISTS logs_by_run ON logs(run, at);
             CREATE TABLE IF NOT EXISTS shots (run TEXT NOT NULL, n INTEGER NOT NULL, at INTEGER NOT NULL, label TEXT NOT NULL, PRIMARY KEY (run, n));
             CREATE TABLE IF NOT EXISTS schedule (automation TEXT PRIMARY KEY, last_day TEXT NOT NULL);",
        )
        .map_err(fail)?;
        Ok(Self { db: Mutex::new(conn), shots: dir.join("shots") })
    }

    fn db(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A new run, before its machine starts. `token` is the hash of the run's secret.
    pub fn begin(&self, id: &str, automation: &str, bot: &str, token: &str, trigger: &str) -> ResultsResult<()> {
        self.db()
            .execute("INSERT INTO runs (id, automation, bot, token, started, status, trigger) VALUES (?1, ?2, ?3, ?4, ?5, 'starting', ?6)", params![id, automation, bot, token, now_ms(), trigger])
            .map(|_| ())
            .map_err(fail)
    }

    pub fn by_token(&self, token: &str) -> Option<RunRef> {
        self.db()
            .query_row("SELECT id, automation, bot, finished FROM runs WHERE token = ?1", params![token], |r| {
                Ok(RunRef { id: r.get(0)?, automation: r.get(1)?, bot: r.get(2)?, finished: r.get::<_, Option<i64>>(3)?.is_some() })
            })
            .optional()
            .ok()
            .flatten()
    }

    /// The script said hello: its machine is up and it is running.
    pub fn running(&self, run: &str) -> ResultsResult<()> {
        self.db().execute("UPDATE runs SET status = 'running' WHERE id = ?1 AND finished IS NULL", params![run]).map(|_| ()).map_err(fail)
    }

    pub fn step(&self, run: &str, line: i64, said: &str) -> ResultsResult<()> {
        self.db()
            .execute("UPDATE runs SET status = 'running', line = ?2, said = ?3 WHERE id = ?1 AND finished IS NULL", params![run, line, said])
            .map(|_| ())
            .map_err(fail)
    }

    pub fn log(&self, run: &str, line: i64, text: &str) -> ResultsResult<()> {
        let db = self.db();
        let count: i64 = db.query_row("SELECT COUNT(*) FROM logs WHERE run = ?1", params![run], |r| r.get(0)).map_err(fail)?;
        if count >= MAX_LOG_LINES {
            return Ok(());
        }
        db.execute("INSERT INTO logs (run, at, line, text) VALUES (?1, ?2, ?3, ?4)", params![run, now_ms(), line, text]).map(|_| ()).map_err(fail)
    }

    pub fn rows(&self, run: &RunRef, dataset: &str, rows: &[Value]) -> ResultsResult<usize> {
        let mut db = self.db();
        let have: i64 = db.query_row("SELECT COUNT(*) FROM rows WHERE run = ?1", params![run.id], |r| r.get(0)).map_err(fail)?;
        let room = (MAX_ROWS - have).max(0) as usize;
        let tx = db.transaction().map_err(fail)?;
        let at = now_ms();
        for row in rows.iter().take(room) {
            tx.execute("INSERT INTO rows (run, automation, dataset, at, data) VALUES (?1, ?2, ?3, ?4, ?5)", params![run.id, run.automation, dataset, at, row.to_string()]).map_err(fail)?;
        }
        tx.commit().map_err(fail)?;
        Ok(rows.len().min(room))
    }

    pub fn metric(&self, run: &RunRef, name: &str, value: f64, better: &str) -> ResultsResult<()> {
        if !value.is_finite() {
            return Err("that is not a number".into());
        }
        self.db()
            .execute("INSERT INTO metrics (run, automation, name, at, value, better) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![run.id, run.automation, name, now_ms(), value, better])
            .map(|_| ())
            .map_err(fail)
    }

    pub fn shot(&self, run: &str, label: &str, png: &[u8]) -> ResultsResult<()> {
        if !png.starts_with(b"\x89PNG") || png.len() > MAX_SHOT_BYTES {
            return Err("a picture must be a PNG under 6 MB".into());
        }
        let db = self.db();
        let n: i64 = db.query_row("SELECT COALESCE(MAX(n), 0) + 1 FROM shots WHERE run = ?1", params![run], |r| r.get(0)).map_err(fail)?;
        if n > MAX_SHOTS {
            return Ok(());
        }
        std::fs::write(self.shot_file(run, n), png).map_err(fail)?;
        db.execute("INSERT INTO shots (run, n, at, label) VALUES (?1, ?2, ?3, ?4)", params![run, n, now_ms(), label]).map(|_| ()).map_err(fail)
    }

    fn shot_file(&self, run: &str, n: i64) -> PathBuf {
        self.shots.join(format!("{run}-{n}.png"))
    }

    /// A picture of a run, by "<run>/<n>". Run ids are made by the box, so the name is checked.
    pub fn shot_bytes(&self, path: &str) -> ResultsResult<Vec<u8>> {
        let (run, n) = path.split_once('/').ok_or("no such picture")?;
        let n: i64 = n.parse().map_err(|_| "no such picture")?;
        if run.is_empty() || !run.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err("no such picture".into());
        }
        std::fs::read(self.shot_file(run, n)).map_err(|_| "no such picture".into())
    }

    /// Ends a run that has not ended: `done`, `failed` or `stopped`. True when this call ended
    /// it. A run that finished well gets its update written.
    pub fn end(&self, run: &RunRef, status: &str, line: Option<i64>, said: &str, message: &str) -> ResultsResult<bool> {
        let changed = {
            let db = self.db();
            db.execute(
                "UPDATE runs SET finished = ?2, status = ?3, line = COALESCE(?4, line), said = CASE WHEN ?5 = '' THEN said ELSE ?5 END, message = ?6 WHERE id = ?1 AND finished IS NULL",
                params![run.id, now_ms(), status, line, said, message],
            )
            .map_err(fail)?
        };
        if changed == 0 {
            return Ok(false);
        }
        if status == "done" {
            let update = self.update_for(&run.automation)?;
            self.db().execute("UPDATE runs SET update_text = ?2 WHERE id = ?1", params![run.id, update]).map_err(fail)?;
        }
        Ok(true)
    }

    /// The bot's newest run, if it has not ended.
    pub fn open_run(&self, automation: &str, bot: &str) -> Option<RunRef> {
        self.db()
            .query_row(
                "SELECT id, automation, bot FROM runs WHERE automation = ?1 AND bot = ?2 AND finished IS NULL ORDER BY started DESC LIMIT 1",
                params![automation, bot],
                |r| Ok(RunRef { id: r.get(0)?, automation: r.get(1)?, bot: r.get(2)?, finished: false }),
            )
            .optional()
            .ok()
            .flatten()
    }

    /// Everything an automation's results screen shows.
    pub fn overview(&self, automation: &str) -> ResultsResult<Value> {
        let db = self.db();
        let mut stmt = db
            .prepare("SELECT id, bot, started, finished, status, line, said, message, update_text, trigger FROM runs WHERE automation = ?1 ORDER BY started DESC LIMIT ?2")
            .map_err(fail)?;
        let mut runs: Vec<Value> = stmt
            .query_map(params![automation, RUNS_SHOWN], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?, "bot": r.get::<_, String>(1)?, "started": r.get::<_, i64>(2)?,
                    "finished": r.get::<_, Option<i64>>(3)?, "status": r.get::<_, String>(4)?, "line": r.get::<_, i64>(5)?,
                    "said": r.get::<_, String>(6)?, "message": r.get::<_, String>(7)?, "update": r.get::<_, String>(8)?,
                    "trigger": r.get::<_, String>(9)?,
                }))
            })
            .map_err(fail)?
            .flatten()
            .collect();
        for run in runs.iter_mut() {
            let id = run["id"].as_str().unwrap_or_default().to_string();
            let mut shots = db.prepare("SELECT n, label FROM shots WHERE run = ?1 ORDER BY n").map_err(fail)?;
            let list: Vec<Value> = shots.query_map(params![id], |r| Ok(json!({ "n": r.get::<_, i64>(0)?, "label": r.get::<_, String>(1)? }))).map_err(fail)?.flatten().collect();
            run["shots"] = json!(list);
            let rows: i64 = db.query_row("SELECT COUNT(*) FROM rows WHERE run = ?1", params![id], |r| r.get(0)).map_err(fail)?;
            run["rows"] = json!(rows);
        }

        // Each dataset as the newest run that posted it left it.
        let mut stmt = db.prepare("SELECT dataset, run, MAX(at) FROM rows WHERE automation = ?1 GROUP BY dataset ORDER BY MIN(at)").map_err(fail)?;
        let newest: Vec<(String, String, i64)> = stmt.query_map(params![automation], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map_err(fail)?.flatten().collect();
        let mut datasets = Vec::new();
        for (name, _, _) in newest {
            let run: String = db
                .query_row("SELECT run FROM rows WHERE automation = ?1 AND dataset = ?2 ORDER BY at DESC LIMIT 1", params![automation, name], |r| r.get(0))
                .map_err(fail)?;
            let mut rows = db.prepare("SELECT data, at FROM rows WHERE run = ?1 AND dataset = ?2 ORDER BY rowid").map_err(fail)?;
            let all: Vec<(String, i64)> = rows.query_map(params![run, name], |r| Ok((r.get(0)?, r.get(1)?))).map_err(fail)?.flatten().collect();
            let at = all.first().map(|(_, at)| *at).unwrap_or(0);
            let total = all.len();
            let parsed: Vec<Value> = all.into_iter().take(ROWS_SHOWN).filter_map(|(d, _)| serde_json::from_str(&d).ok()).collect();
            let mut columns: Vec<String> = Vec::new();
            for row in &parsed {
                if let Some(obj) = row.as_object() {
                    for k in obj.keys() {
                        if !columns.contains(k) {
                            columns.push(k.clone());
                        }
                    }
                }
            }
            datasets.push(json!({ "name": name, "run": run, "at": at, "total": total, "columns": columns, "rows": parsed }));
        }

        let since = now_ms() - DAYS_SHOWN * DAY_MS;
        let mut stmt = db.prepare("SELECT name, at, value, better FROM metrics WHERE automation = ?1 AND at >= ?2 ORDER BY at").map_err(fail)?;
        let points: Vec<(String, i64, f64, String)> = stmt.query_map(params![automation, since], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(fail)?.flatten().collect();
        let mut by_name: BTreeMap<String, (String, Vec<Value>)> = BTreeMap::new();
        for (name, at, value, better) in points {
            let entry = by_name.entry(name).or_insert_with(|| (better.clone(), Vec::new()));
            entry.0 = better;
            entry.1.push(json!({ "at": at, "value": value }));
        }
        let metrics: Vec<Value> = by_name.into_iter().map(|(name, (better, points))| json!({ "name": name, "better": better, "points": points })).collect();

        let latest = runs.first().and_then(|r| r["id"].as_str()).map(str::to_string);
        let log: Vec<Value> = match &latest {
            Some(run) => {
                let mut stmt = db.prepare("SELECT at, line, text FROM (SELECT rowid, at, line, text FROM logs WHERE run = ?1 ORDER BY rowid DESC LIMIT 120) ORDER BY rowid").map_err(fail)?;
                stmt.query_map(params![run], |r| Ok(json!({ "at": r.get::<_, i64>(0)?, "line": r.get::<_, i64>(1)?, "text": r.get::<_, String>(2)? }))).map_err(fail)?.flatten().collect()
            }
            None => Vec::new(),
        };
        let update = runs.iter().find_map(|r| r["update"].as_str().filter(|u| !u.is_empty()).map(str::to_string)).unwrap_or_default();
        Ok(json!({ "runs": runs, "datasets": datasets, "metrics": metrics, "log": log, "update": update }))
    }

    /// The newest run of an automation, short: for its card in the list.
    pub fn latest(&self, automation: &str) -> Option<Value> {
        let db = self.db();
        let run = db
            .query_row(
                "SELECT id, bot, started, finished, status, line, said, message FROM runs WHERE automation = ?1 ORDER BY started DESC LIMIT 1",
                params![automation],
                |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?, "bot": r.get::<_, String>(1)?, "started": r.get::<_, i64>(2)?,
                        "finished": r.get::<_, Option<i64>>(3)?, "status": r.get::<_, String>(4)?, "line": r.get::<_, i64>(5)?,
                        "said": r.get::<_, String>(6)?, "message": r.get::<_, String>(7)?,
                    }))
                },
            )
            .optional()
            .ok()
            .flatten()?;
        let update: String = db
            .query_row("SELECT update_text FROM runs WHERE automation = ?1 AND update_text != '' ORDER BY started DESC LIMIT 1", params![automation], |r| r.get(0))
            .optional()
            .ok()
            .flatten()
            .unwrap_or_default();
        Some(json!({ "run": run, "update": update }))
    }

    /// Everything an automation left, for when it is removed.
    pub fn forget(&self, automation: &str) -> ResultsResult<()> {
        let db = self.db();
        let mut stmt = db.prepare("SELECT run, n FROM shots WHERE run IN (SELECT id FROM runs WHERE automation = ?1)").map_err(fail)?;
        let files: Vec<(String, i64)> = stmt.query_map(params![automation], |r| Ok((r.get(0)?, r.get(1)?))).map_err(fail)?.flatten().collect();
        for (run, n) in files {
            let _ = std::fs::remove_file(self.shot_file(&run, n));
        }
        db.execute_batch("BEGIN").map_err(fail)?;
        for sql in [
            "DELETE FROM shots WHERE run IN (SELECT id FROM runs WHERE automation = ?1)",
            "DELETE FROM logs WHERE run IN (SELECT id FROM runs WHERE automation = ?1)",
            "DELETE FROM rows WHERE automation = ?1",
            "DELETE FROM metrics WHERE automation = ?1",
            "DELETE FROM runs WHERE automation = ?1",
            "DELETE FROM schedule WHERE automation = ?1",
        ] {
            db.execute(sql, params![automation]).map_err(fail)?;
        }
        db.execute_batch("COMMIT").map_err(fail)
    }

    pub fn last_scheduled(&self, automation: &str) -> Option<String> {
        self.db().query_row("SELECT last_day FROM schedule WHERE automation = ?1", params![automation], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_last_scheduled(&self, automation: &str, day: &str) -> ResultsResult<()> {
        self.db()
            .execute("INSERT INTO schedule (automation, last_day) VALUES (?1, ?2) ON CONFLICT(automation) DO UPDATE SET last_day = ?2", params![automation, day])
            .map(|_| ())
            .map_err(fail)
    }

    /// The update in words for an automation's tracked values, from every day it has.
    fn update_for(&self, automation: &str) -> ResultsResult<String> {
        let db = self.db();
        let mut stmt = db.prepare("SELECT name, at, value, better FROM metrics WHERE automation = ?1 ORDER BY at").map_err(fail)?;
        let points: Vec<(String, i64, f64, String)> = stmt.query_map(params![automation], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).map_err(fail)?.flatten().collect();
        let mut by_name: BTreeMap<String, (String, BTreeMap<NaiveDate, f64>)> = BTreeMap::new();
        for (name, at, value, better) in points {
            let entry = by_name.entry(name).or_default();
            entry.0 = better;
            // A day counts once: its last reading.
            entry.1.insert(local_day(at), value);
        }
        let parts: Vec<String> = by_name.into_iter().map(|(name, (better, days))| advice(&name, &better, &days.into_iter().collect::<Vec<_>>())).collect();
        Ok(parts.join(" "))
    }
}

/// "$6.39" for a price, "6.39" for anything else.
fn shown(name: &str, value: f64) -> String {
    let lower = name.to_lowercase();
    if lower.contains("price") || lower.contains("cost") {
        format!("${value:.2}")
    } else if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn capital(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|c| c.to_uppercase().collect::<String>() + chars.as_str()).unwrap_or_default()
}

fn weekday_name(day: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match day {
        Mon => "Monday",
        Tue => "Tuesday",
        Wed => "Wednesday",
        Thu => "Thursday",
        Fri => "Friday",
        Sat => "Saturday",
        Sun => "Sunday",
    }
}

/// What a value's days say about today, in a sentence or two. `days` is in date order, one
/// value a day, the last one today's. `better` is "lower", "higher" or "".
pub fn advice(name: &str, better: &str, days: &[(NaiveDate, f64)]) -> String {
    let Some(&(today, now)) = days.last() else { return String::new() };
    let title = capital(name);
    let before: Vec<(NaiveDate, f64)> = days[..days.len() - 1].iter().copied().filter(|(d, _)| (today - *d).num_days() <= 30).collect();
    if before.is_empty() {
        let first = if days.len() == 1 { "the first day Castle has" } else { "the first day in a month" };
        return format!("{title}: {} today. This is {first}, so it cannot say yet whether that is good. A few more days and it can.", shown(name, now));
    }
    let span = (today - before[0].0).num_days().max(1);
    let avg = before.iter().map(|(_, v)| v).sum::<f64>() / before.len() as f64;
    let pct = if avg != 0.0 { (now - avg) / avg.abs() * 100.0 } else { 0.0 };
    let (best_day, best) = before.iter().copied().fold(before[0], |acc, p| match better {
        "higher" => if p.1 > acc.1 { p } else { acc },
        _ => if p.1 < acc.1 { p } else { acc },
    });
    let avg_text = format!("the {span}-day average of {}", shown(name, avg));
    let gap = pct.abs().round();
    let mut text = match better {
        "lower" if now <= best => format!("Buy today: {} is the lowest in {span} days, {gap}% under {avg_text}.", shown(name, now)),
        "lower" if pct <= -NOTABLE_PERCENT => format!("Good day to buy: {}, {gap}% under {avg_text}.", shown(name, now)),
        "lower" if pct >= NOTABLE_PERCENT => format!("Wait if you can: {} is {gap}% over {avg_text}. It was {} on {}.", shown(name, now), shown(name, best), best_day.format("%b %-d")),
        "higher" if now >= best => format!("Best day yet: {} is the highest in {span} days, {gap}% over {avg_text}.", shown(name, now)),
        "higher" if pct >= NOTABLE_PERCENT => format!("Good day: {}, {gap}% over {avg_text}.", shown(name, now)),
        "higher" if pct <= -NOTABLE_PERCENT => format!("A low day: {} is {gap}% under {avg_text}. It was {} on {}.", shown(name, now), shown(name, best), best_day.format("%b %-d")),
        "lower" | "higher" => format!("About usual: {}, close to {avg_text}.", shown(name, now)),
        _ if gap < 1.0 => format!("{title}: {}, close to {avg_text}.", shown(name, now)),
        _ => format!("{title}: {}, {gap}% {} {avg_text}.", shown(name, now), if pct > 0.0 { "over" } else { "under" }),
    };
    if !text.starts_with(&title) && better != "" {
        text = format!("{title} — {}{}", text[..1].to_lowercase(), &text[1..]);
    }

    // Over two weeks or more, the weekday that has been best on average.
    if better != "" && days.len() >= WEEKDAY_MIN_DAYS {
        let mut by_day: BTreeMap<u32, Vec<f64>> = BTreeMap::new();
        for (d, v) in days {
            by_day.entry(d.weekday().num_days_from_monday()).or_default().push(*v);
        }
        let overall = days.iter().map(|(_, v)| v).sum::<f64>() / days.len() as f64;
        let means: Vec<(u32, f64)> = by_day.iter().filter(|(_, vs)| vs.len() >= 2).map(|(d, vs)| (*d, vs.iter().sum::<f64>() / vs.len() as f64)).collect();
        let pick = if better == "higher" { means.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)) } else { means.iter().copied().min_by(|a, b| a.1.total_cmp(&b.1)) };
        if let Some((d, mean)) = pick {
            let off = (mean - overall) / overall.abs().max(f64::EPSILON) * 100.0;
            let notable = if better == "higher" { off >= WEEKDAY_PERCENT } else { off <= -WEEKDAY_PERCENT };
            if notable {
                let day = chrono::Weekday::try_from(d as u8).map(weekday_name).unwrap_or("");
                let word = if better == "higher" { "highest" } else { "lowest" };
                text.push_str(&format!(" Over {} days it has been {word} on {day}s.", days.len()));
            }
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(n: i64) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 1).unwrap() + chrono::Duration::days(n)
    }

    #[test]
    fn the_first_day_only_says_what_it_is() {
        let text = advice("average price per quart", "lower", &[(day(0), 6.39)]);
        assert!(text.starts_with("Average price per quart: $6.39 today. This is the first day"));
    }

    #[test]
    fn a_new_low_says_buy_today() {
        let days = [(day(0), 6.8), (day(1), 6.7), (day(2), 6.9), (day(3), 6.2)];
        let text = advice("average price per quart", "lower", &days);
        assert!(text.contains("buy today: $6.20 is the lowest in 3 days"), "{text}");
        assert!(text.contains("under the 3-day average of $6.80"), "{text}");
    }

    #[test]
    fn a_high_day_says_wait_and_names_the_low() {
        let days = [(day(0), 6.0), (day(1), 6.1), (day(2), 6.6)];
        let text = advice("average price per quart", "lower", &days);
        assert!(text.contains("wait if you can: $6.60 is 9% over"), "{text}");
        assert!(text.contains("It was $6.00 on Nov 1"), "{text}");
    }

    #[test]
    fn close_to_usual_says_so() {
        let days = [(day(0), 6.0), (day(1), 6.2), (day(2), 6.12)];
        assert!(advice("average price per quart", "lower", &days).contains("about usual: $6.12"));
    }

    #[test]
    fn two_weeks_find_the_cheapest_weekday() {
        // Nov 1 2026 is a Sunday: Tuesdays (n = 2, 9) are cheap.
        let days: Vec<(NaiveDate, f64)> = (0..15).map(|n| (day(n), if n % 7 == 2 { 5.5 } else { 6.5 })).collect();
        let text = advice("price", "lower", &days);
        assert!(text.ends_with("Over 15 days it has been lowest on Tuesdays."), "{text}");
    }

    #[test]
    fn a_value_with_no_direction_is_only_compared() {
        let days = [(day(0), 10.0), (day(1), 12.0)];
        assert_eq!(advice("listings found", "", &days), "Listings found: 12, 20% over the 1-day average of 10.");
    }

    #[test]
    fn runs_rows_and_updates_round_trip() {
        let dir = std::env::temp_dir().join(format!("castle-results-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let results = Results::open(&dir).unwrap();
        results.begin("r1", "oil", "main", "hash1", "start").unwrap();
        let run = results.by_token("hash1").unwrap();
        assert_eq!((run.id.as_str(), run.finished), ("r1", false));
        results.step("r1", 7, "website walmart.com").unwrap();
        assert_eq!(results.rows(&run, "listings", &[json!({ "title": "A", "price": 6.0 }), json!({ "title": "B" })]).unwrap(), 2);
        results.metric(&run, "average price per quart", 6.39, "lower").unwrap();
        assert!(results.end(&run, "done", None, "", "").unwrap());
        assert!(!results.end(&run, "failed", Some(3), "x", "too late").unwrap(), "a run ends once");
        let o = results.overview("oil").unwrap();
        assert_eq!(o["runs"][0]["status"], "done");
        assert_eq!(o["runs"][0]["line"], 7);
        assert_eq!(o["datasets"][0]["columns"], json!(["title", "price"]));
        assert!(o["update"].as_str().unwrap().contains("$6.39 today"));
        assert!(results.by_token("hash1").unwrap().finished);
        results.forget("oil").unwrap();
        assert_eq!(results.overview("oil").unwrap()["runs"], json!([]));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn picture_names_cannot_leave_the_folder() {
        let dir = std::env::temp_dir().join(format!("castle-results-shot-{}", std::process::id()));
        let results = Results::open(&dir).unwrap();
        assert!(results.shot_bytes("../../etc/1").is_err());
        assert!(results.shot_bytes("r1/x").is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
