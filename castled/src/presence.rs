//! Presence: what is moving in the house, from small radar nodes on the walls.
//!
//! A node is an ESP32 with a 24 GHz mmWave sensor (a Hi-Link LD2450, which tracks up to
//! three targets by position, or an LD2410, which only says whether someone is there). It
//! sits by a doorway or in a room and tells the box, several times a second, what it sees:
//! present or not, and where each target is in millimetres from the sensor (x across, y
//! straight out) with its speed. The app draws that as a motion-tracker sweep and a list of
//! rooms, occupied or empty. There is no camera anywhere in this: the box senses, it does
//! not watch.
//!
//! Keys: each node has its own key, made when an admin adds it, shown once, and kept here
//! only as a hash. A node's key can report what that node sees and nothing else, so a
//! board on the wall never holds the box token. Replacing a node's key locks the old one
//! out at once.
//!
//! What is kept: the list of nodes (name, room, sensor, where it sits on the house plan,
//! the key's hash) is saved in the vault's private folder. What the nodes report is kept
//! in memory only: the latest reading of each node, and a short list of rooms being
//! entered and left. A restart forgets where everyone was.
//!
//! Limits, said plainly: presence is not identity (it says something alive is there, not
//! who); pets, fans and moving curtains show up until the sensor is tuned; and radar passes
//! wood and drywall but not much brick or concrete, so this is an indoor sense, not a fence.

use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub type PresenceResult<T> = Result<T, String>;

pub const SENSORS: [&str; 2] = ["ld2450", "ld2410"];
const MAX_NODES: usize = 64;
const MAX_NAME_LEN: usize = 60;
/// An LD2450 tracks at most three targets.
const MAX_TARGETS: usize = 3;
/// The sensors see about 6 m; a report far past that is not a real reading.
const MAX_RANGE_MM: i32 = 10_000;
/// 10 m/s, in the sensor's centimetres a second. Faster than any person indoors.
const MAX_SPEED: i32 = 1_000;
const MAX_FIRMWARE_LEN: usize = 40;
/// A reading counts for this long. A node that stops reporting stops showing targets.
const TARGET_LIFE_MS: u64 = 3_000;
/// A node quiet for this long is offline (unplugged, out of Wi-Fi, or broken).
const OFFLINE_AFTER_MS: u64 = 30_000;
/// The least time between two reports from one node: about 12 a second at most.
const MIN_REPORT_GAP_MS: u64 = 80;
const MAX_EVENTS: usize = 200;
/// The largest report body the box reads. A full LD2450 report is about 200 bytes.
pub const MAX_REPORT_BYTES: usize = 2048;
const ID_BYTES: usize = 6;
const SECRET_BYTES: usize = 24;

fn now_millis() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("the operating system provides randomness");
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_secret(secret: &str) -> String {
    Sha256::digest(secret.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// A name or room as typed: trimmed, single-spaced, not empty, not too long.
fn clean_label(text: &str, what: &str) -> PresenceResult<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() {
        return Err(format!("the {what} is empty"));
    }
    if text.chars().count() > MAX_NAME_LEN || text.chars().any(char::is_control) {
        return Err(format!("that {what} is too long"));
    }
    Ok(text)
}

// ---------------------------------------------------------------------------------------
// What is kept, what is reported, what the app is shown
// ---------------------------------------------------------------------------------------

/// Where a node sits on the house plan, so its sweep can be drawn in the right place:
/// metres from the plan's corner, and which way the sensor faces, in degrees.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
}

impl Placement {
    fn checked(self) -> PresenceResult<Self> {
        let fine = |v: f64, limit: f64| v.is_finite() && v.abs() <= limit;
        if !fine(self.x, 1_000.0) || !fine(self.y, 1_000.0) || !self.rotation.is_finite() {
            return Err("that placement is off the plan".into());
        }
        Ok(Self { rotation: self.rotation.rem_euclid(360.0), ..self })
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Stored {
    id: String,
    name: String,
    room: String,
    sensor: String,
    key_hash: String,
    #[serde(default)]
    placement: Placement,
    added: u64,
}

/// One thing a sensor sees: millimetres across (x, negative is left) and out (y) from the
/// sensor, and speed in centimetres a second (negative is coming closer).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub speed: i32,
}

/// What a node sends. `present` defaults to "a target was seen", so an LD2450 can send
/// only its targets and an LD2410 only `present`.
#[derive(Deserialize, Default)]
pub struct Report {
    #[serde(default)]
    pub present: Option<bool>,
    #[serde(default)]
    pub targets: Vec<Target>,
    #[serde(default)]
    pub firmware: Option<String>,
}

#[derive(Default)]
struct Live {
    /// When the last report arrived; 0 when none has since the box started.
    last_seen: u64,
    present: bool,
    targets: Vec<Target>,
    reports: u64,
    firmware: String,
    /// Whether the node was online at the last look, to note when it drops off or returns.
    online: bool,
}

impl Live {
    fn online(&self, now: u64) -> bool {
        self.last_seen > 0 && now.saturating_sub(self.last_seen) <= OFFLINE_AFTER_MS
    }

    fn fresh(&self, now: u64) -> bool {
        self.last_seen > 0 && now.saturating_sub(self.last_seen) <= TARGET_LIFE_MS
    }
}

/// Something that happened: a room was entered or left, a node went offline or came back.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub at: u64,
    pub kind: String,
    pub room: String,
    pub node: String,
    pub node_name: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NodeView {
    pub id: String,
    pub name: String,
    pub room: String,
    pub sensor: String,
    pub placement: Placement,
    pub added: u64,
    pub online: bool,
    pub last_seen: Option<u64>,
    pub present: bool,
    pub targets: Vec<Target>,
    pub reports: u64,
    pub firmware: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomView {
    pub name: String,
    pub occupied: bool,
    /// When it became occupied or empty, as far as the box has seen since it started.
    pub since: Option<u64>,
    pub nodes: Vec<String>,
    /// Some node in the room is reporting; when none is, the room's state is unknown.
    pub watched: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub nodes: Vec<NodeView>,
    pub rooms: Vec<RoomView>,
    /// Newest first.
    pub events: Vec<Event>,
    pub now: u64,
    pub target_life: u64,
    pub offline_after: u64,
}

/// A new node and its key, which is shown once and never again.
#[derive(Serialize)]
pub struct Added {
    pub node: NodeView,
    pub key: String,
}

#[derive(Deserialize)]
pub struct AddBody {
    pub name: String,
    pub room: String,
    #[serde(default = "default_sensor")]
    pub sensor: String,
    #[serde(default)]
    pub placement: Option<Placement>,
}

fn default_sensor() -> String {
    SENSORS[0].into()
}

#[derive(Deserialize)]
pub struct UpdateBody {
    pub id: String,
    pub name: Option<String>,
    pub room: Option<String>,
    pub sensor: Option<String>,
    pub placement: Option<Placement>,
}

/// Why a report was turned away.
#[derive(Debug, PartialEq)]
pub enum Refused {
    /// No such node, or the wrong key.
    Key,
    /// Sooner than MIN_REPORT_GAP_MS after the last one.
    TooSoon,
    Bad(String),
}

// ---------------------------------------------------------------------------------------
// The presence keeper
// ---------------------------------------------------------------------------------------

#[derive(Default)]
struct Inner {
    nodes: Vec<Stored>,
    live: HashMap<String, Live>,
    /// room -> (occupied, since)
    rooms: HashMap<String, (bool, Option<u64>)>,
    events: VecDeque<Event>,
}

pub struct Presence {
    file: PathBuf,
    inner: Mutex<Inner>,
}

impl Presence {
    pub fn open(dir: PathBuf) -> Self {
        let file = dir.join("nodes.json");
        let nodes: Vec<Stored> = std::fs::read(&file).ok().and_then(|raw| serde_json::from_slice(&raw).ok()).unwrap_or_default();
        Self { file, inner: Mutex::new(Inner { nodes, ..Inner::default() }) }
    }

    fn lock(&self) -> PresenceResult<std::sync::MutexGuard<'_, Inner>> {
        self.inner.lock().map_err(|_| "presence is unavailable".to_string())
    }

    fn save(&self, nodes: &[Stored]) -> PresenceResult<()> {
        let fail = |_| "could not save the node list on the box".to_string();
        if let Some(dir) = self.file.parent() {
            std::fs::create_dir_all(dir).map_err(fail)?;
        }
        let tmp = self.file.with_extension("tmp");
        let raw = serde_json::to_vec_pretty(nodes).map_err(|_| "could not save the node list on the box".to_string())?;
        std::fs::write(&tmp, raw).map_err(fail)?;
        std::fs::rename(&tmp, &self.file).map_err(fail)
    }

    pub fn overview(&self) -> PresenceResult<Overview> {
        self.overview_at(now_millis())
    }

    pub fn overview_at(&self, now: u64) -> PresenceResult<Overview> {
        let mut inner = self.lock()?;
        settle(&mut inner, now);
        let nodes = inner.nodes.iter().map(|n| view(n, inner.live.get(&n.id), now)).collect::<Vec<_>>();
        let mut names: Vec<&str> = inner.nodes.iter().map(|n| n.room.as_str()).collect();
        names.sort_unstable_by_key(|r| r.to_lowercase());
        names.dedup();
        let rooms = names
            .into_iter()
            .map(|room| {
                let (occupied, since) = inner.rooms.get(room).copied().unwrap_or((false, None));
                let members: Vec<&Stored> = inner.nodes.iter().filter(|n| n.room == room).collect();
                RoomView {
                    name: room.to_string(),
                    occupied,
                    since,
                    nodes: members.iter().map(|n| n.id.clone()).collect(),
                    watched: members.iter().any(|n| inner.live.get(&n.id).is_some_and(|l| l.online(now))),
                }
            })
            .collect();
        Ok(Overview {
            nodes,
            rooms,
            events: inner.events.iter().rev().cloned().collect(),
            now,
            target_life: TARGET_LIFE_MS,
            offline_after: OFFLINE_AFTER_MS,
        })
    }

    pub fn add(&self, body: AddBody) -> PresenceResult<Added> {
        let name = clean_label(&body.name, "name")?;
        let room = clean_label(&body.room, "room")?;
        if !SENSORS.contains(&body.sensor.as_str()) {
            return Err("that sensor is not one Castle knows".into());
        }
        let placement = body.placement.unwrap_or_default().checked()?;
        let mut inner = self.lock()?;
        if inner.nodes.len() >= MAX_NODES {
            return Err(format!("a box keeps at most {MAX_NODES} nodes"));
        }
        if inner.nodes.iter().any(|n| n.name.eq_ignore_ascii_case(&name)) {
            return Err("a node already has that name".into());
        }
        let id = random_hex(ID_BYTES);
        let secret = random_hex(SECRET_BYTES);
        let stored = Stored { id: id.clone(), name, room, sensor: body.sensor, key_hash: hash_secret(&secret), placement, added: now_millis() };
        let mut nodes = inner.nodes.clone();
        nodes.push(stored.clone());
        self.save(&nodes)?;
        inner.nodes = nodes;
        Ok(Added { node: view(&stored, None, now_millis()), key: format!("{id}.{secret}") })
    }

    pub fn update(&self, body: UpdateBody) -> PresenceResult<NodeView> {
        let mut inner = self.lock()?;
        let mut nodes = inner.nodes.clone();
        let at = nodes.iter().position(|n| n.id == body.id).ok_or("no such node")?;
        if let Some(name) = body.name {
            let name = clean_label(&name, "name")?;
            if nodes.iter().any(|n| n.id != body.id && n.name.eq_ignore_ascii_case(&name)) {
                return Err("a node already has that name".into());
            }
            nodes[at].name = name;
        }
        if let Some(room) = body.room {
            nodes[at].room = clean_label(&room, "room")?;
        }
        if let Some(sensor) = body.sensor {
            if !SENSORS.contains(&sensor.as_str()) {
                return Err("that sensor is not one Castle knows".into());
            }
            nodes[at].sensor = sensor;
        }
        if let Some(placement) = body.placement {
            nodes[at].placement = placement.checked()?;
        }
        self.save(&nodes)?;
        inner.nodes = nodes;
        let now = now_millis();
        settle(&mut inner, now);
        let node = &inner.nodes[at];
        Ok(view(node, inner.live.get(&node.id), now))
    }

    pub fn remove(&self, id: &str) -> PresenceResult<()> {
        let mut inner = self.lock()?;
        let nodes: Vec<Stored> = inner.nodes.iter().filter(|n| n.id != id).cloned().collect();
        if nodes.len() == inner.nodes.len() {
            return Err("no such node".into());
        }
        self.save(&nodes)?;
        inner.nodes = nodes;
        inner.live.remove(id);
        settle(&mut inner, now_millis());
        Ok(())
    }

    /// A new key for a node; the old one stops working at once.
    pub fn rekey(&self, id: &str) -> PresenceResult<String> {
        let mut inner = self.lock()?;
        let mut nodes = inner.nodes.clone();
        let node = nodes.iter_mut().find(|n| n.id == id).ok_or("no such node")?;
        let secret = random_hex(SECRET_BYTES);
        node.key_hash = hash_secret(&secret);
        self.save(&nodes)?;
        inner.nodes = nodes;
        Ok(format!("{id}.{secret}"))
    }

    /// A node's reading. `key` is the whole node key, "<node id>.<secret>"; `read` makes the
    /// report from the request, and is called only once the key has been accepted.
    pub fn report_with(&self, key: &str, read: impl FnOnce() -> PresenceResult<Report>) -> Result<(), Refused> {
        self.report_with_at(key, read, now_millis())
    }

    #[cfg(test)]
    fn report_at(&self, key: &str, report: Report, now: u64) -> Result<(), Refused> {
        self.report_with_at(key, || Ok(report), now)
    }

    fn report_with_at(&self, key: &str, read: impl FnOnce() -> PresenceResult<Report>, now: u64) -> Result<(), Refused> {
        let (id, secret) = key.split_once('.').ok_or(Refused::Key)?;
        let mut inner = self.inner.lock().map_err(|_| Refused::Bad("presence is unavailable".into()))?;
        let node = inner.nodes.iter().find(|n| n.id == id).ok_or(Refused::Key)?;
        // Constant-time, so how long the answer takes does not say how much of a guess was right.
        let matches: bool = hash_secret(secret).as_bytes().ct_eq(node.key_hash.as_bytes()).into();
        if !matches {
            return Err(Refused::Key);
        }
        let report = read().map_err(Refused::Bad)?;
        check(&report).map_err(Refused::Bad)?;
        let live = inner.live.entry(id.to_string()).or_default();
        if live.last_seen > 0 && now.saturating_sub(live.last_seen) < MIN_REPORT_GAP_MS {
            return Err(Refused::TooSoon);
        }
        live.last_seen = now;
        live.present = report.present.unwrap_or(!report.targets.is_empty());
        live.targets = report.targets;
        live.reports += 1;
        if let Some(firmware) = report.firmware {
            live.firmware = firmware.chars().filter(|c| !c.is_control()).take(MAX_FIRMWARE_LEN).collect();
        }
        settle(&mut inner, now);
        Ok(())
    }
}

/// A report the sensors could really have made, or why not.
fn check(report: &Report) -> PresenceResult<()> {
    if report.targets.len() > MAX_TARGETS {
        return Err(format!("a node reports at most {MAX_TARGETS} targets"));
    }
    for t in &report.targets {
        if t.x.abs() > MAX_RANGE_MM || t.y.abs() > MAX_RANGE_MM {
            return Err("a target is out of the sensor's range".into());
        }
        if t.speed.abs() > MAX_SPEED {
            return Err("a target is moving faster than anything indoors".into());
        }
    }
    Ok(())
}

fn view(node: &Stored, live: Option<&Live>, now: u64) -> NodeView {
    let fresh = live.is_some_and(|l| l.fresh(now));
    NodeView {
        id: node.id.clone(),
        name: node.name.clone(),
        room: node.room.clone(),
        sensor: node.sensor.clone(),
        placement: node.placement,
        added: node.added,
        online: live.is_some_and(|l| l.online(now)),
        last_seen: live.map(|l| l.last_seen).filter(|t| *t > 0),
        present: fresh && live.is_some_and(|l| l.present),
        targets: if fresh { live.map(|l| l.targets.clone()).unwrap_or_default() } else { Vec::new() },
        reports: live.map_or(0, |l| l.reports),
        firmware: live.map(|l| l.firmware.clone()).unwrap_or_default(),
    }
}

fn push(inner: &mut Inner, event: Event) {
    inner.events.push_back(event);
    while inner.events.len() > MAX_EVENTS {
        inner.events.pop_front();
    }
}

/// Brings rooms and nodes up to `now`: notes nodes going offline or coming back, and rooms
/// becoming occupied or empty. A room is occupied while any of its nodes has a fresh
/// reading of someone there.
fn settle(inner: &mut Inner, now: u64) {
    let mut events = Vec::new();
    for node in &inner.nodes {
        let Some(live) = inner.live.get_mut(&node.id) else { continue };
        let online = live.online(now);
        if online != live.online {
            live.online = online;
            events.push(Event {
                at: if online { live.last_seen } else { live.last_seen + OFFLINE_AFTER_MS },
                kind: if online { "online" } else { "offline" }.into(),
                room: node.room.clone(),
                node: node.id.clone(),
                node_name: node.name.clone(),
            });
        }
    }
    let mut rooms: Vec<&str> = inner.nodes.iter().map(|n| n.room.as_str()).collect();
    rooms.sort_unstable();
    rooms.dedup();
    for room in rooms {
        let seeing = inner.nodes.iter().filter(|n| n.room == room).find(|n| inner.live.get(&n.id).is_some_and(|l| l.fresh(now) && l.present));
        let occupied = seeing.is_some();
        let (was, _) = inner.rooms.get(room).copied().unwrap_or((false, None));
        if occupied == was {
            continue;
        }
        // Who noticed: the node that sees someone, or for an empty room the one that saw last.
        let by = seeing.or_else(|| inner.nodes.iter().filter(|n| n.room == room).max_by_key(|n| inner.live.get(&n.id).map_or(0, |l| l.last_seen)));
        let at = if occupied { now } else { by.and_then(|n| inner.live.get(&n.id)).map_or(now, |l| (l.last_seen + TARGET_LIFE_MS).min(now)) };
        events.push(Event {
            at,
            kind: if occupied { "entered" } else { "left" }.into(),
            room: room.to_string(),
            node: by.map(|n| n.id.clone()).unwrap_or_default(),
            node_name: by.map(|n| n.name.clone()).unwrap_or_default(),
        });
        inner.rooms.insert(room.to_string(), (occupied, Some(at)));
    }
    // Rooms no node is in any more are forgotten.
    let known: Vec<String> = inner.nodes.iter().map(|n| n.room.clone()).collect();
    inner.rooms.retain(|room, _| known.contains(room));
    events.sort_by_key(|e| e.at);
    for event in events {
        push(inner, event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> (PathBuf, Presence) {
        let dir = std::env::temp_dir().join(format!("castled-presence-{name}-{}", now_millis()));
        let _ = std::fs::remove_dir_all(&dir);
        (dir.clone(), Presence::open(dir))
    }

    fn add(p: &Presence, name: &str, room: &str) -> Added {
        p.add(AddBody { name: name.into(), room: room.into(), sensor: "ld2450".into(), placement: None }).unwrap()
    }

    fn seen(x: i32, y: i32) -> Report {
        Report { targets: vec![Target { x, y, speed: 0 }], ..Report::default() }
    }

    #[test]
    fn only_the_nodes_own_key_reports() {
        let (dir, p) = fixture("keys");
        let hall = add(&p, "Hall door", "Hall");
        let den = add(&p, "Den", "Den");
        assert!(p.report_at(&hall.key, seen(100, 900), 1_000).is_ok());
        // Another node's secret with this node's id, a made-up key, a bare id: all refused.
        let (hall_id, _) = hall.key.split_once('.').unwrap();
        let (_, den_secret) = den.key.split_once('.').unwrap();
        assert_eq!(p.report_at(&format!("{hall_id}.{den_secret}"), seen(0, 1), 2_000), Err(Refused::Key));
        assert_eq!(p.report_at("nope.nope", seen(0, 1), 2_000), Err(Refused::Key));
        assert_eq!(p.report_at(hall_id, seen(0, 1), 2_000), Err(Refused::Key));
        // The key is kept only as a hash.
        let saved = std::fs::read_to_string(dir.join("nodes.json")).unwrap();
        assert!(!saved.contains(hall.key.split_once('.').unwrap().1));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_new_key_locks_the_old_one_out() {
        let (dir, p) = fixture("rekey");
        let node = add(&p, "Kitchen", "Kitchen");
        let fresh = p.rekey(&node.node.id).unwrap();
        assert_eq!(p.report_at(&node.key, seen(0, 500), 1_000), Err(Refused::Key));
        assert!(p.report_at(&fresh, seen(0, 500), 1_000).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn nodes_survive_a_restart_but_readings_do_not() {
        let (dir, p) = fixture("restart");
        let node = add(&p, "Hall door", "Hall");
        p.report_at(&node.key, seen(0, 800), 1_000).unwrap();
        let again = Presence::open(dir.clone());
        let o = again.overview_at(1_500).unwrap();
        assert_eq!(o.nodes.len(), 1);
        assert!(!o.nodes[0].present && o.nodes[0].targets.is_empty());
        assert!(again.report_at(&node.key, seen(0, 800), 2_000).is_ok(), "the key still works");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn reports_that_no_sensor_could_make_are_refused() {
        let (dir, p) = fixture("bad");
        let node = add(&p, "Den", "Den");
        let four = Report { targets: vec![Target { x: 0, y: 1, speed: 0 }; 4], ..Report::default() };
        assert!(matches!(p.report_at(&node.key, four, 1_000), Err(Refused::Bad(_))));
        assert!(matches!(p.report_at(&node.key, seen(0, 50_000), 1_000), Err(Refused::Bad(_))));
        let fast = Report { targets: vec![Target { x: 0, y: 1, speed: 5_000 }], ..Report::default() };
        assert!(matches!(p.report_at(&node.key, fast, 1_000), Err(Refused::Bad(_))));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_node_cannot_flood_the_box() {
        let (dir, p) = fixture("flood");
        let node = add(&p, "Den", "Den");
        assert!(p.report_at(&node.key, seen(0, 500), 1_000).is_ok());
        assert_eq!(p.report_at(&node.key, seen(0, 500), 1_020), Err(Refused::TooSoon));
        assert!(p.report_at(&node.key, seen(0, 500), 1_000 + MIN_REPORT_GAP_MS).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rooms_are_entered_and_left_and_quiet_nodes_go_offline() {
        let (dir, p) = fixture("rooms");
        let a = add(&p, "Hall door", "Hall");
        let b = add(&p, "Hall end", "Hall");
        p.report_at(&a.key, seen(200, 1_200), 10_000).unwrap();
        p.report_at(&b.key, Report::default(), 10_000).unwrap();
        let o = p.overview_at(10_500).unwrap();
        assert!(o.rooms[0].occupied && o.rooms[0].watched);
        assert_eq!(o.nodes.iter().find(|n| n.id == a.node.id).unwrap().targets.len(), 1);
        assert_eq!(o.events.iter().filter(|e| e.kind == "entered").count(), 1);

        // Nobody seen any more: the targets fade, the room empties once, by the node that saw last.
        p.report_at(&a.key, Report::default(), 12_000).unwrap();
        let o = p.overview_at(12_100).unwrap();
        assert!(!o.rooms[0].occupied);
        let left: Vec<&Event> = o.events.iter().filter(|e| e.kind == "left").collect();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].room, "Hall");

        // A stale reading stops counting even if it said someone was there.
        p.report_at(&a.key, seen(0, 900), 13_000).unwrap();
        assert!(p.overview_at(13_100).unwrap().rooms[0].occupied);
        let o = p.overview_at(13_000 + TARGET_LIFE_MS + 1).unwrap();
        assert!(!o.rooms[0].occupied && o.nodes.iter().all(|n| n.targets.is_empty()));

        // Quiet for long enough: offline, once each, and the room is no longer watched.
        let o = p.overview_at(13_000 + OFFLINE_AFTER_MS + 1).unwrap();
        assert!(o.nodes.iter().all(|n| !n.online));
        assert!(!o.rooms[0].watched);
        assert_eq!(o.events.iter().filter(|e| e.kind == "offline").count(), 2);
        let o = p.overview_at(13_000 + OFFLINE_AFTER_MS + 5_000).unwrap();
        assert_eq!(o.events.iter().filter(|e| e.kind == "offline").count(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn names_rooms_and_placement_are_checked() {
        let (dir, p) = fixture("labels");
        let made = add(&p, "  Front   door ", " Hall ");
        assert_eq!((made.node.name.as_str(), made.node.room.as_str()), ("Front door", "Hall"));
        assert!(p.add(AddBody { name: "front door".into(), room: "Hall".into(), sensor: "ld2450".into(), placement: None }).is_err(), "names are unique");
        assert!(p.add(AddBody { name: "".into(), room: "Hall".into(), sensor: "ld2450".into(), placement: None }).is_err());
        assert!(p.add(AddBody { name: "x".into(), room: "Hall".into(), sensor: "camera".into(), placement: None }).is_err());
        let moved = p
            .update(UpdateBody { id: made.node.id.clone(), name: None, room: Some("Porch".into()), sensor: None, placement: Some(Placement { x: 2.5, y: 1.0, rotation: -90.0 }) })
            .unwrap();
        assert_eq!(moved.room, "Porch");
        assert_eq!(moved.placement.rotation, 270.0);
        let off = Placement { x: f64::NAN, y: 0.0, rotation: 0.0 };
        assert!(p.update(UpdateBody { id: made.node.id.clone(), name: None, room: None, sensor: None, placement: Some(off) }).is_err());
        p.remove(&made.node.id).unwrap();
        assert!(p.overview_at(1).unwrap().nodes.is_empty());
        assert!(p.remove(&made.node.id).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }
}
