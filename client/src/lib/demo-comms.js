// The demo's Comms: sample texts, picture messages, calls, email and chats for each demo
// person, in the shapes the box answers (castled/src/comms.rs). Times are relative to now,
// so "Today" and "Yesterday" are always filled. The people line up with Contacts' demo:
// Dad, Mom, Ana López, Grandma Rose, Jordan and Sam.

const MIN = 60 * 1000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
/** The demo pages smaller than the box (400) so "Load older" shows up. */
const PAGE = 40;

const DAD = '+1 555 010 2030';
const MOM = '+1 555 010 4455';
const ANA = '+1 555 777 0000';
const ROSE = '+1 555 222 1961';
const JORDAN = '+1 555 300 1188';
const SAM = '+1 555 300 2299';

/** A picture for the demo's picture messages: a drawn scene, no real photo. */
function scene(sky, ground, sun, label) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="480" viewBox="0 0 640 480">
<defs><linearGradient id="s" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${sky[0]}"/><stop offset="1" stop-color="${sky[1]}"/></linearGradient></defs>
<rect width="640" height="480" fill="url(#s)"/><circle cx="470" cy="150" r="64" fill="${sun}" opacity="0.9"/>
<path d="M0 330 Q160 270 320 320 T640 300 V480 H0Z" fill="${ground[0]}"/><path d="M0 390 Q200 340 400 390 T640 380 V480 H0Z" fill="${ground[1]}"/>
<text x="24" y="456" font-family="sans-serif" font-size="22" fill="rgba(255,255,255,0.75)">${label}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/** One second of silence, as a WAV, for the demo's call recording. */
function silence() {
  const rate = 8000;
  const bytes = new Uint8Array(44 + rate);
  const view = new DataView(bytes.buffer);
  const text = (at, s) => [...s].forEach((c, i) => view.setUint8(at + i, c.charCodeAt(0)));
  text(0, 'RIFF');
  view.setUint32(4, 36 + rate, true);
  text(8, 'WAVEfmt ');
  view.setUint32(16, 16, true);
  view.setUint16(20, 1, true);
  view.setUint16(22, 1, true);
  view.setUint32(24, rate, true);
  view.setUint32(28, rate, true);
  view.setUint16(32, 1, true);
  view.setUint16(34, 8, true);
  text(36, 'data');
  view.setUint32(40, rate, true);
  bytes.fill(128, 44); // 8-bit silence sits at the middle
  return bytes;
}

const FILES = {
  'demo-beach.svg': { mime: 'image/svg+xml', url: () => scene(['#3b82c4', '#f5c48a'], ['#d9b37a', '#c49a5e'], '#fff3c4', 'Lake day') },
  'demo-garden.svg': { mime: 'image/svg+xml', url: () => scene(['#9cc9e8', '#e8f1d8'], ['#5c8f3a', '#3f6e27'], '#ffe58a', 'Grandma’s garden') },
  'demo-cake.svg': { mime: 'image/svg+xml', url: () => scene(['#2a1838', '#6b3f63'], ['#8a4b2d', '#5e2f1b'], '#ffd27a', 'Birthday, 65 candles') },
  'demo-recording.wav': { mime: 'audio/wav', url: () => URL.createObjectURL(new Blob([silence()], { type: 'audio/wav' })) },
  'demo-itinerary.pdf': { mime: 'application/pdf', url: () => `data:application/pdf;base64,${btoa('%PDF-1.4\n% Castle demo: a pretend itinerary\n')}` },
};

const att = (file, name) => ({ name, mime: FILES[file].mime, file, size: file.endsWith('.wav') ? 8044 : 24_000 });

const capitalise = (w) => (w ? w[0].toUpperCase() + w.slice(1) : '');

function duration(s) {
  if (s >= 3600) return `${Math.floor(s / 3600)}h ${Math.floor((s % 3600) / 60)}m`;
  if (s >= 60) return `${Math.floor(s / 60)}m ${s % 60}s`;
  return `${s}s`;
}

/** The line a conversation shows under its name, as the box makes it. */
function snippet(m) {
  let text = '';
  if (m.call) text = `${capitalise(m.call.direction)} call${m.call.seconds ? `, ${duration(m.call.seconds)}` : ''}`;
  else if (m.subject) text = m.subject;
  else if (m.body) text = m.body;
  else if (m.attachments?.[0]) text = m.attachments[0].mime.startsWith('image/') ? 'Sent a picture' : `Sent ${m.attachments[0].name}`;
  text = text.replace(/[\r\n]+/g, ' ');
  return text.length > 140 ? `${text.slice(0, 140)}…` : text;
}

/** Builds one person's archive from a short script of conversations. */
function archive(now, script) {
  let n = 0;
  const conversations = [];
  const threads = new Map();
  for (const conv of script) {
    const id = `demo${String(conversations.length).padStart(16, '0')}`;
    const messages = conv.messages
      .map((m) => ({ id: `m${n++}`, at: now - m.ago, from: m.me ? 'Me' : m.from || conv.title, address: m.me ? '' : m.address || conv.participants[0] || '', me: !!m.me, kind: m.kind || conv.kind, subject: m.subject || '', body: m.body || '', call: m.call || null, attachments: m.attachments || [] }))
      .sort((a, b) => a.at - b.at);
    threads.set(id, messages);
    conversations.push({ id, channel: conv.channel, title: conv.title, participants: conv.participants, count: messages.length, first: messages[0].at, last: messages.at(-1).at, snippet: snippet(messages.at(-1)) });
  }
  return { conversations, threads };
}

/** Dad texts most days: enough for "Load older" to have something to load. */
function dadDaily() {
  const lines = [
    ['Did you see the game?', 'Missed it, working late'],
    ['Weather looks good for Saturday', 'Lake?'],
    ['Your mother says call her', 'Will do tonight'],
    ['Found your old bike in the garage', 'Keep it for Sam!'],
    ['How is the new box working out', 'Holds everything. Even your voicemails.'],
    ['Grandma wants the recipe back', 'It is in the shared folder'],
    ['Oil change done', '👍'],
    ['Fish were not biting', 'Next time'],
  ];
  const out = [];
  for (let d = 40; d >= 3; d--) {
    const [his, mine] = lines[d % lines.length];
    out.push({ ago: d * DAY + 9 * HOUR, body: his }, { ago: d * DAY + 8 * HOUR + 40 * MIN, me: true, body: mine });
  }
  return out;
}

function alexScript() {
  return [
    {
      channel: 'texts', kind: 'sms', title: 'Dad', participants: [DAD],
      messages: [
        ...dadDaily(),
        { ago: 2 * DAY + 3 * HOUR, body: 'Dinner Sunday? Mom is making the lasagna' },
        { ago: 2 * DAY + 2 * HOUR, me: true, body: 'We will be there at 6' },
        { ago: 26 * HOUR, kind: 'mms', body: 'Look what we caught', attachments: [att('demo-beach.svg', 'IMG_2231.jpg')] },
        { ago: 25 * HOUR, me: true, body: 'That is a big one. Saving this to the box 😄' },
        { ago: 3 * HOUR, body: 'Can you send me that wifi password again' },
        { ago: 2 * HOUR + 50 * MIN, me: true, body: 'It is on the card on the fridge. Castle-Guest, the long one.' },
      ],
    },
    {
      channel: 'texts', kind: 'mms', title: 'Family', participants: [DAD, MOM, ANA],
      messages: [
        { ago: 6 * DAY, from: 'Mom', address: MOM, body: 'Grandma turns 65 on the 19th! Party at ours' },
        { ago: 6 * DAY - 10 * MIN, from: 'Ana López', address: ANA, body: 'I will bring the cake' },
        { ago: 6 * DAY - 30 * MIN, me: true, body: 'I have the photo slideshow covered. Every picture since 1985 is on the box now.' },
        { ago: 4 * DAY, from: 'Dad', address: DAD, body: 'Garden pictures from Rose', attachments: [att('demo-garden.svg', 'garden.jpg')] },
        { ago: 20 * HOUR, from: 'Ana López', address: ANA, body: 'Test run of the cake', attachments: [att('demo-cake.svg', 'cake.jpg')] },
        { ago: 19 * HOUR, from: 'Mom', address: MOM, body: 'Perfect ❤️' },
        { ago: 40 * MIN, me: true, body: 'Leaving in 10' },
      ],
    },
    {
      channel: 'texts', kind: 'sms', title: 'Grandma Rose', participants: [ROSE],
      messages: [
        { ago: 9 * DAY, body: 'Thank you for the flowers dear' },
        { ago: 9 * DAY - HOUR, me: true, body: 'Happy early birthday, Grandma' },
        { ago: 5 * HOUR, body: 'Is the video of the 1994 reunion on your computer thing? I would love to see it' },
        { ago: 4 * HOUR, me: true, body: 'It is in Theater, under Home videos. I will put it on the TV Sunday.' },
      ],
    },
    {
      channel: 'calls', kind: 'call', title: 'Dad', participants: [DAD],
      messages: [
        { ago: 8 * DAY, call: { seconds: 0, direction: 'missed' } },
        { ago: 8 * DAY - 20 * MIN, me: true, call: { seconds: 312, direction: 'outgoing' } },
        { ago: 2 * DAY + 4 * HOUR, call: { seconds: 1265, direction: 'incoming' }, attachments: [att('demo-recording.wav', 'Call recording Dad_1113_1832.wav')] },
        { ago: 6 * HOUR, call: { seconds: 0, direction: 'voicemail' } },
      ],
    },
    {
      channel: 'calls', kind: 'call', title: 'Grandma Rose', participants: [ROSE],
      messages: [
        { ago: 12 * DAY, call: { seconds: 1840, direction: 'incoming' } },
        { ago: 3 * DAY, me: true, call: { seconds: 655, direction: 'outgoing' } },
      ],
    },
    {
      channel: 'calls', kind: 'call', title: '+1 555 404 0000', participants: ['+1 555 404 0000'],
      messages: [{ ago: 30 * HOUR, call: { seconds: 0, direction: 'rejected' } }],
    },
    {
      channel: 'email', kind: 'email', title: 'Dad', participants: ['dad@example.com'],
      messages: [
        { ago: 15 * DAY, from: 'Dad', address: 'dad@example.com', subject: 'Cabin trip', body: 'Booked the cabin for the long weekend. Itinerary attached.\n\nBring the good cooler.\n\nDad', attachments: [att('demo-itinerary.pdf', 'cabin-itinerary.pdf')] },
        { ago: 15 * DAY - 2 * HOUR, me: true, subject: 'Re: Cabin trip', body: 'We are in. Jordan is bringing the board games.\n\n> Booked the cabin for the long weekend. Itinerary attached.' },
      ],
    },
    {
      channel: 'email', kind: 'email', title: 'Ana López', participants: ['ana@example.com'],
      messages: [
        {
          ago: 7 * DAY, from: 'Ana López', address: 'ana@example.com', subject: 'The family recipes, finally typed up',
          body: 'Hi!\n\nI went through Grandma Rose’s recipe box over the weekend and typed every card up. There are forty-one of them, from the lasagna to the lemon cake nobody has managed to get right since 1998.\n\nA few notes:\n\n1. Some cards say "a cup" and mean her blue cup, which is about three quarters of a real one. I wrote both.\n2. The garden soup has no amounts at all. I watched her make it and guessed.\n3. Two cards were in Grandpa’s handwriting. I kept them as photos too.\n\nCould you keep these on the box? I trust it more than my laptop, which has died twice this year.\n\nLove,\nAna',
        },
        { ago: 7 * DAY - 3 * HOUR, me: true, subject: 'Re: The family recipes, finally typed up', body: 'Done. They are in Storage under Family › Recipes, and the box keeps every version, so nobody can lose the lemon cake again.' },
      ],
    },
    {
      channel: 'email', kind: 'email', title: 'Northside Library', participants: ['notices@library.example'],
      messages: [{ ago: 4 * DAY, from: 'Northside Library', address: 'notices@library.example', subject: 'Your holds are ready', body: 'Two items are waiting for you at the front desk until Friday.' }],
    },
    {
      channel: 'discord', kind: 'chat', title: 'Cruz Family · #game-night', participants: ['Jordan', 'Sam'],
      messages: [
        { ago: 3 * DAY, from: 'Jordan', address: 'jordan', body: 'who is up for game night friday' },
        { ago: 3 * DAY - 5 * MIN, from: 'Sam', address: 'sam', body: 'ME' },
        { ago: 3 * DAY - 8 * MIN, me: true, body: 'The Steam cache has the new one already, so no waiting on downloads' },
        { ago: 3 * DAY - 9 * MIN, from: 'Jordan', address: 'jordan', body: 'castle carrying the family once again' },
        { ago: 2 * DAY, from: 'Sam', address: 'sam', body: 'gg everyone [screenshot.png]' },
      ],
    },
    {
      channel: 'facebook', kind: 'chat', title: 'Ana López', participants: ['Ana López'],
      messages: [
        { ago: 400 * DAY, from: 'Ana López', body: 'Found you on here! How is everyone?' },
        { ago: 400 * DAY - HOUR, me: true, body: 'Ana! All good. Sam starts school next week.' },
        { ago: 390 * DAY, from: 'Ana López', kind: 'call', call: { seconds: 840, direction: 'incoming' } },
        { ago: 120 * DAY, from: 'Ana López', body: 'Moving my messages off here, see you on regular texts 🙂' },
      ],
    },
  ];
}

function jordanScript() {
  return [
    {
      channel: 'texts', kind: 'sms', title: 'Sam', participants: [SAM],
      messages: [
        { ago: 5 * HOUR, body: 'can you pick me up after practice' },
        { ago: 4 * HOUR + 50 * MIN, me: true, body: 'Yes, 5:30 at the side door' },
      ],
    },
    {
      channel: 'texts', kind: 'sms', title: 'Mom', participants: [MOM],
      messages: [
        { ago: 2 * DAY, body: 'Bring a jacket Sunday, it gets cold by the lake' },
        { ago: 2 * DAY - 20 * MIN, me: true, body: 'Will do' },
      ],
    },
    {
      channel: 'calls', kind: 'call', title: 'Mom', participants: [MOM],
      messages: [{ ago: DAY, me: true, call: { seconds: 422, direction: 'outgoing' } }],
    },
    {
      channel: 'discord', kind: 'chat', title: 'Cruz Family · #game-night', participants: ['Alex', 'Sam'],
      messages: [
        { ago: 3 * DAY, me: true, body: 'who is up for game night friday' },
        { ago: 3 * DAY - 5 * MIN, from: 'Sam', address: 'sam', body: 'ME' },
        { ago: 3 * DAY - 9 * MIN, me: true, body: 'castle carrying the family once again' },
      ],
    },
  ];
}

function samScript() {
  return [
    {
      channel: 'texts', kind: 'sms', title: 'Jordan', participants: [JORDAN],
      messages: [
        { ago: 5 * HOUR, me: true, body: 'can you pick me up after practice' },
        { ago: 4 * HOUR + 50 * MIN, body: 'Yes, 5:30 at the side door' },
      ],
    },
    {
      channel: 'discord', kind: 'chat', title: 'Cruz Family · #game-night', participants: ['Alex', 'Jordan'],
      messages: [
        { ago: 3 * DAY, from: 'Jordan', address: 'jordan', body: 'who is up for game night friday' },
        { ago: 3 * DAY - 5 * MIN, me: true, body: 'ME' },
        { ago: 2 * DAY, me: true, body: 'gg everyone [screenshot.png]' },
      ],
    },
  ];
}

const SCRIPTS = { alex: alexScript, jordan: jordanScript, sam: samScript };

const IMPORTS = (now) => [
  { at: now - 2 * HOUR, source: 'sms-20261113.xml', report: { texts: 14, calls: 0, emails: 0, chats: 0, recordings: 0, attachments: 3, known: 2108, conversations: 1, ignored: 0, contactsFiles: 0 } },
  { at: now - 2 * HOUR + 5 * MIN, source: 'calls-20261113.xml', report: { texts: 0, calls: 6, emails: 0, chats: 0, recordings: 0, attachments: 0, known: 412, conversations: 1, ignored: 0, contactsFiles: 0 } },
  { at: now - 9 * DAY, source: 'takeout-20261104T120000Z-001.zip', report: { texts: 0, calls: 0, emails: 3891, chats: 0, recordings: 0, attachments: 642, known: 0, conversations: 517, ignored: 22, contactsFiles: 1 } },
  { at: now - 30 * DAY, source: 'facebook-alexcruz.zip', report: { texts: 0, calls: 1, emails: 0, chats: 2210, recordings: 0, attachments: 188, known: 0, conversations: 64, ignored: 310, contactsFiles: 0 } },
];

export class DemoComms {
  constructor() {
    this.now = Date.now();
    this.people = new Map(); // person -> { conversations, threads, imports }
    this.urls = new Map();
    this.job = null;
    this.problem = new Map();
  }

  /** One person's archive, made the first time they open Comms. */
  of(person) {
    if (!this.people.has(person)) {
      const script = SCRIPTS[person];
      const made = script ? archive(this.now, script()) : { conversations: [], threads: new Map() };
      made.imports = person === 'alex' ? IMPORTS(this.now) : [];
      this.people.set(person, made);
    }
    return this.people.get(person);
  }

  async comms(person) {
    const a = this.of(person);
    return {
      conversations: [...a.conversations].sort((x, y) => y.last - x.last),
      imports: [...a.imports].sort((x, y) => y.at - x.at),
      importing: this.job?.person === person ? { ...this.job } : null,
      problem: this.problem.get(person) || null,
    };
  }

  async commsThread(person, id, before = null) {
    const a = this.of(person);
    const conversation = a.conversations.find((c) => c.id === id);
    if (!conversation) throw new Error('no such conversation');
    const all = a.threads.get(id) || [];
    const upto = before ? all.findIndex((m) => m.at >= before) : -1;
    const end = upto === -1 ? all.length : upto;
    const from = Math.max(0, end - PAGE);
    return { conversation, messages: all.slice(from, end), more: from > 0 };
  }

  async commsSearch(person, q) {
    const words = String(q || '').toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return [];
    const a = this.of(person);
    const hits = [];
    for (const c of [...a.conversations].sort((x, y) => y.last - x.last)) {
      for (const m of [...(a.threads.get(c.id) || [])].reverse()) {
        const hay = `${m.body} ${m.subject} ${m.from} ${m.attachments.map((x) => x.name).join(' ')}`.toLowerCase();
        if (words.every((w) => hay.includes(w))) hits.push({ conversation: c.id, message: m });
      }
    }
    return hits.slice(0, 200);
  }

  async commsFile(person, name) {
    if (!FILES[name]) return null;
    if (!this.urls.has(name)) this.urls.set(name, FILES[name].url());
    return this.urls.get(name);
  }

  async commsDelete(person, id) {
    const a = this.of(person);
    a.conversations = a.conversations.filter((c) => c.id !== id);
    a.threads.delete(id);
  }

  /** The demo cannot read a real export: it pretends to work on it, then says so. */
  async commsImport(person, file) {
    if (this.job) throw new Error('an import is already going; wait for it to finish');
    this.problem.delete(person);
    this.job = { person, source: file?.name || 'export', started: Date.now(), step: 'Reading' };
    setTimeout(() => {
      if (this.job) this.job.step = 'Putting conversations together';
    }, 1500);
    setTimeout(() => {
      this.job = null;
      this.problem.set(person, `The demo does not read files, so ${file?.name || 'that file'} was not opened. On your own box, its messages land here.`);
    }, 3200);
    return { started: true };
  }
}
