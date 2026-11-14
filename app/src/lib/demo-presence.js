// Demo stand-in for the box's presence nodes (castled/src/presence.rs): a few radar nodes
// around a pretend house, with people walking past them on loops, so Security > Presence
// has blips to show. It answers the same calls as the box and keeps rooms and events the
// same way: a room is occupied while one of its nodes saw someone in the last few seconds.

const TARGET_LIFE = 3000;
const OFFLINE_AFTER = 30000;
const MAX_EVENTS = 200;
const MIN = 60 * 1000;
const NEW_NODE_JOINS_AFTER = 4000; // a node added in the demo "boots" and joins this soon

/**
 * A walk past a node: from `from` to `to` (millimetres from the sensor), taking `during`
 * of every `period` milliseconds, starting `offset` into it, with a sideways sway.
 */
const walk = (from, to, period, during, offset = 0, sway = 120) => ({ from, to, period, during, offset, sway });

const HOUSE = [
  {
    id: 'demo-front',
    name: 'Front door',
    room: 'Hall',
    walks: [walk([-1600, 4800], [900, 700], 22000, 0.32), walk([1800, 5200], [-400, 1400], 37000, 0.25, 9000)],
  },
  {
    id: 'demo-living',
    name: 'Living room',
    room: 'Living room',
    walks: [walk([-900, 2600], [-700, 2500], 9000, 1, 0, 60), walk([1200, 900], [2200, 4300], 26000, 0.45, 4000)],
  },
  {
    id: 'demo-kitchen',
    name: 'Kitchen',
    room: 'Kitchen',
    walks: [walk([-1400, 1200], [1500, 1600], 15000, 0.4, 2000, 200)],
  },
  {
    id: 'demo-stairs',
    name: 'Stairs',
    room: 'Hall',
    walks: [walk([0, 5600], [300, 800], 41000, 0.2, 15000, 60)],
  },
  { id: 'demo-garage', name: 'Garage door', room: 'Garage', walks: [], offline: true },
];

const speedTowards = (a, b, seconds) => Math.round((Math.hypot(...b) - Math.hypot(...a)) / 10 / seconds); // cm/s, negative is closer

/** Where each walker on a node is at time `t`, as the sensor would report it. */
function targetsAt(node, t) {
  const out = [];
  for (const w of node.walks) {
    const phase = (((t + w.offset) % w.period) + w.period) % w.period / w.period;
    if (phase > w.during) continue;
    const k = w.during === 1 ? 0.5 + 0.5 * Math.sin((2 * Math.PI * t) / w.period) : phase / w.during;
    const at = (f) => [w.from[0] + (w.to[0] - w.from[0]) * f, w.from[1] + (w.to[1] - w.from[1]) * f];
    const [x, y] = at(k);
    const sway = w.sway * Math.sin(t / 700 + w.offset);
    const step = w.during === 1 ? 0 : 1 / ((w.period * w.during) / 1000); // fraction of the walk per second
    const speed = w.during === 1 ? 0 : speedTowards(at(k), at(Math.min(1, k + step)), 1);
    out.push({ x: Math.round(x + sway), y: Math.round(Math.max(150, y)), speed });
  }
  return out.slice(0, 3);
}

export class DemoPresence {
  isDemo = true;

  constructor() {
    const now = Date.now();
    this.nodes = HOUSE.map((n, i) => ({
      ...n,
      sensor: 'ld2450',
      placement: { x: i * 3, y: 0, rotation: 0 },
      added: now - (40 - i) * 24 * 60 * MIN,
      firmware: 'gatehouse-ld2450/0.1',
      born: n.offline ? null : now - 3 * 60 * MIN,
      lastSeen: n.offline ? now - 52 * MIN : null,
    }));
    this.rooms = new Map();
    this.events = [
      { at: now - 52 * MIN, kind: 'offline', room: 'Garage', node: 'demo-garage', nodeName: 'Garage door' },
      { at: now - 47 * MIN, kind: 'entered', room: 'Kitchen', node: 'demo-kitchen', nodeName: 'Kitchen' },
      { at: now - 41 * MIN, kind: 'left', room: 'Kitchen', node: 'demo-kitchen', nodeName: 'Kitchen' },
      { at: now - 18 * MIN, kind: 'entered', room: 'Living room', node: 'demo-living', nodeName: 'Living room' },
      { at: now - 6 * MIN, kind: 'entered', room: 'Hall', node: 'demo-front', nodeName: 'Front door' },
      { at: now - 5 * MIN, kind: 'left', room: 'Hall', node: 'demo-front', nodeName: 'Front door' },
    ];
    this.keys = 0;
  }

  reading(node, now) {
    const online = node.born != null && now >= node.born;
    const targets = online ? targetsAt(node, now) : [];
    return { online, targets, lastSeen: online ? now - 80 : node.lastSeen };
  }

  settle(now) {
    for (const room of new Set(this.nodes.map((n) => n.room))) {
      const members = this.nodes.filter((n) => n.room === room);
      const seeing = members.find((n) => this.reading(n, now).targets.length);
      const occupied = !!seeing;
      const was = this.rooms.get(room);
      if (was && was.occupied === occupied) continue;
      this.rooms.set(room, { occupied, since: was ? now : this.events.findLast((e) => e.room === room && (e.kind === 'entered' || e.kind === 'left'))?.at ?? null });
      if (!was) continue; // the first look only learns where things stand
      const by = seeing || members[0];
      this.events.push({ at: now, kind: occupied ? 'entered' : 'left', room, node: by.id, nodeName: by.name });
      if (this.events.length > MAX_EVENTS) this.events.shift();
    }
  }

  async presence() {
    const now = Date.now();
    this.settle(now);
    const nodes = this.nodes.map((n) => {
      const r = this.reading(n, now);
      return {
        id: n.id,
        name: n.name,
        room: n.room,
        sensor: n.sensor,
        placement: n.placement,
        added: n.added,
        online: r.online,
        lastSeen: r.lastSeen,
        present: r.targets.length > 0,
        targets: n.sensor === 'ld2450' ? r.targets : [],
        reports: r.online ? Math.round((now - n.born) / 150) : 0,
        firmware: r.online || r.lastSeen ? n.firmware : '',
      };
    });
    const rooms = [...new Set(this.nodes.map((n) => n.room))]
      .sort((a, b) => a.localeCompare(b))
      .map((name) => {
        const members = nodes.filter((n) => n.room === name);
        const state = this.rooms.get(name) || { occupied: false, since: null };
        return { name, occupied: state.occupied, since: state.since, nodes: members.map((n) => n.id), watched: members.some((n) => n.online) };
      });
    return { nodes, rooms, events: [...this.events].reverse(), now, targetLife: TARGET_LIFE, offlineAfter: OFFLINE_AFTER };
  }

  newKey(id) {
    this.keys += 1;
    const secret = Array.from({ length: 48 }, (_, i) => '0123456789abcdef'[(i * 7 + this.keys * 13) % 16]).join('');
    return `${id}.${secret}`;
  }

  async presenceNodeAdd({ name, room, sensor = 'ld2450' }) {
    const clean = (s) => (s || '').trim().replace(/\s+/g, ' ');
    if (!clean(name) || !clean(room)) throw new Error(`The ${clean(name) ? 'room' : 'name'} is empty`);
    if (this.nodes.some((n) => n.name.toLowerCase() === clean(name).toLowerCase())) throw new Error('A node already has that name');
    const id = `demo-${Date.now().toString(16)}`;
    const now = Date.now();
    const node = {
      id,
      name: clean(name),
      room: clean(room),
      sensor,
      placement: { x: 0, y: 0, rotation: 0 },
      added: now,
      firmware: 'gatehouse-ld2450/0.1',
      born: now + NEW_NODE_JOINS_AFTER,
      lastSeen: null,
      walks: [walk([-1500, 3800], [1500, 1200], 18000, 0.5)],
    };
    this.nodes.push(node);
    const { nodes } = await this.presence();
    return { node: nodes.find((n) => n.id === id), key: this.newKey(id) };
  }

  async presenceNodeUpdate({ id, name, room, sensor }) {
    const node = this.nodes.find((n) => n.id === id);
    if (!node) throw new Error('No such node');
    if (name != null) node.name = name.trim() || node.name;
    if (room != null) node.room = room.trim() || node.room;
    if (sensor) node.sensor = sensor;
    return (await this.presence()).nodes.find((n) => n.id === id);
  }

  async presenceNodeRemove(id) {
    this.nodes = this.nodes.filter((n) => n.id !== id);
    for (const room of [...this.rooms.keys()]) if (!this.nodes.some((n) => n.room === room)) this.rooms.delete(room);
  }

  async presenceNodeRekey(id) {
    if (!this.nodes.some((n) => n.id === id)) throw new Error('No such node');
    return { key: this.newKey(id) };
  }
}
