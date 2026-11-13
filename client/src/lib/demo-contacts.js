// The demo's Contacts: an address book for each demo person, in the shape the box answers
// (castled/src/contacts.rs). Alex's has a dad with home videos on his card, two cards that
// are really one person, birthdays coming up, and the plumber. Nothing is saved: changes
// last for the visit.

import { contactKeys } from './people.js';

const DAY = 24 * 60 * 60 * 1000;

/** A birthday `days` from today, in a given year (or none, for "--MM-DD"). */
function birthdayIn(days, year) {
  const d = new Date(Date.now() + days * DAY);
  const md = `${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`;
  return year ? `${year}-${md}` : `--${md}`;
}

const field = (label, value) => ({ label, value });

/** A face for a card: a soft gradient and the person's initials, as an SVG picture. */
function face(initials, from, to) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="240" height="240" viewBox="0 0 240 240"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs><rect width="240" height="240" fill="url(#g)"/><circle cx="120" cy="96" r="44" fill="rgba(255,255,255,0.28)"/><path d="M40 230c8-52 44-78 80-78s72 26 80 78" fill="rgba(255,255,255,0.28)"/><text x="120" y="226" font-family="system-ui,sans-serif" font-size="30" font-weight="700" fill="rgba(0,0,0,0.45)" text-anchor="middle">${initials}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

function card(id, fields, now) {
  return {
    id,
    name: '',
    given: '',
    family: '',
    nickname: '',
    phones: [],
    emails: [],
    addresses: [],
    urls: [],
    org: '',
    title: '',
    birthday: '',
    notes: '',
    groups: [],
    media: [],
    starred: false,
    photo: '',
    photoAt: 0,
    source: 'Pixel 9 (contacts.vcf)',
    added: now - 400 * DAY,
    modified: now - 30 * DAY,
    ...fields,
  };
}

function alexBook(now) {
  const people = [
    card('dad', {
      name: 'Dad', given: 'Robert', family: 'Cruz', nickname: 'Bob',
      phones: [field('Mobile', '+1 555 010 2030'), field('Home', '+1 555 010 4400')],
      emails: [field('Home', 'dad@example.com'), field('Fishing club', 'bob.cruz@lakeside.example')],
      addresses: [field('Home', '12 Oak St, Springfield, IL 62701')],
      birthday: birthdayIn(9, 1961),
      notes: 'Takes his coffee black. Still has the 1998 truck. Ask about the lake house before July.',
      groups: ['Family'], media: ['video-1', 'video-2', 'photo-0-3', 'photo-0-7'], starred: true,
      photo: 'image/svg+xml', photoAt: now - 60 * DAY,
    }, now),
    card('mom', {
      name: 'Mom', given: 'Linda', family: 'Cruz',
      phones: [field('Mobile', '+1 555 010 4455')], emails: [field('Home', 'linda.cruz@example.com')],
      addresses: [field('Home', '12 Oak St, Springfield, IL 62701')],
      birthday: birthdayIn(41, 1963), notes: 'Garden club Thursdays.', groups: ['Family'],
      media: ['video-0', 'photo-1-2'], starred: true, photo: 'image/svg+xml', photoAt: now - 60 * DAY,
    }, now),
    card('rose', {
      name: 'Grandma Rose', given: 'Rose', family: 'Alvarez',
      phones: [field('Home', '+1 555 222 1961')], birthday: birthdayIn(2, 1939),
      notes: 'Calls on Sundays. Likes the photos printed, not on a phone.', groups: ['Family'], starred: true,
      media: ['video-3'], photo: 'image/svg+xml', photoAt: now - 90 * DAY,
    }, now),
    card('ana', {
      name: 'Ana López', given: 'Ana', family: 'López',
      phones: [field('Mobile', '+1 555 777 0000')], emails: [field('Home', 'ana@example.com')],
      org: 'Springfield Elementary', title: 'Teacher', birthday: birthdayIn(120, 1990), groups: ['Friends'],
      photo: 'image/svg+xml', photoAt: now - 20 * DAY, media: ['photo-0-1'],
    }, now),
    card('jordan', {
      name: 'Jordan', given: 'Jordan', family: 'Cruz',
      phones: [field('Mobile', '+1 555 303 1212')], emails: [field('', 'jordan@example.com')],
      birthday: birthdayIn(200, 1988), groups: ['Family'], starred: true,
    }, now),
    card('sam', { name: 'Sam', given: 'Sam', family: 'Cruz', phones: [field('Mobile', '+1 555 303 4545')], birthday: birthdayIn(63, 2015), groups: ['Family'], media: ['video-0', 'video-3'] }, now),
    // The same people, saved twice from two phones.
    card('bob-old', {
      name: 'Bob Cruz', phones: [field('', '(555) 010-2030')], emails: [field('', 'dad@example.com')],
      source: 'Old Galaxy S9 (backup.vcf)', added: now - 900 * DAY, modified: now - 900 * DAY,
    }, now),
    card('ana-work', {
      name: 'ana lopez', emails: [field('Work', 'ANA@example.com'), field('Work', 'a.lopez@springfield-elem.example')],
      org: 'Springfield Elementary', source: 'Google Contacts (contacts.vcf)', added: now - 700 * DAY,
    }, now),
  ];
  const others = [
    ['Mike (plumber)', '+1 555 410 8800', '', 'Mike’s Pipes', 'Fixed the water heater. Cash or check, no cards.'],
    ['Dr. Patel', '+1 555 600 1200', 'frontdesk@patelfamilydental.example', 'Patel Family Dental', 'Cleanings every 6 months.'],
    ['Coach Rivera', '+1 555 330 0909', 'rivera@springfieldsoccer.example', 'Springfield U10 Soccer', 'Practice Tue/Thu 5:30.'],
    ['Priya Shah', '+1 555 870 2211', 'priya.shah@example.com', '', ''],
    ['Marcus Bell', '+1 555 870 4400', 'marcus@example.com', 'Bell & Sons Roofing', ''],
    ['Tomás Ortega', '+1 555 640 1188', '', '', 'Neighbour at 14 Oak. Has a spare key.'],
    ['Hannah Kim', '+1 555 910 3030', 'hannah.kim@example.com', '', 'Book club.'],
    ['Leo Martin', '+1 555 515 7272', 'leo@example.com', 'Martin Auto', 'Oil change every 5k.'],
    ['Aunt Carmen', '+1 555 222 4848', 'carmen.alvarez@example.com', '', 'Makes the tamales at Christmas.'],
    ['Uncle Dave', '+1 555 222 9090', '', '', ''],
    ['Springfield Vet', '+1 555 800 7387', 'hello@springfieldvet.example', 'Springfield Veterinary', 'Biscuit’s shots due in March.'],
    ['Nina Petrova', '+1 555 404 6611', 'nina.p@example.com', '', ''],
  ];
  others.forEach(([name, phone, email, org, notes], i) => {
    const family = ['Aunt Carmen', 'Uncle Dave'].includes(name);
    people.push(card(`c${i}`, {
      name, phones: [field('Mobile', phone)], emails: email ? [field('', email)] : [], org, notes,
      groups: family ? ['Family'] : org ? ['Services'] : ['Friends'],
      birthday: family ? birthdayIn(20 + i * 17, 1960 + i) : '',
      added: now - (300 - i * 11) * DAY,
    }, now));
  });
  return people;
}

function jordanBook(now) {
  return [
    card('j-alex', { name: 'Alex', given: 'Alex', family: 'Cruz', phones: [field('Mobile', '+1 555 303 1000')], groups: ['Family'], starred: true, photo: 'image/svg+xml', photoAt: now }, now),
    card('j-dad', { name: 'Dad', given: 'Robert', family: 'Cruz', phones: [field('Mobile', '+1 555 010 2030')], birthday: birthdayIn(9, 1961), groups: ['Family'], media: ['video-1'], photo: 'image/svg+xml', photoAt: now }, now),
    card('j-gym', { name: 'Gym', org: 'Iron Works Gym', phones: [field('Work', '+1 555 246 8100')], notes: 'Membership renews in May.' }, now),
    card('j-tara', { name: 'Tara Nguyen', phones: [field('Mobile', '+1 555 707 3131')], emails: [field('', 'tara.n@example.com')], birthday: birthdayIn(5, 1989), groups: ['Friends'] }, now),
  ];
}

function samBook(now) {
  return [
    card('s-mom', { name: 'Mom', phones: [field('Mobile', '+1 555 010 4455')], starred: true, photo: 'image/svg+xml', photoAt: now }, now),
    card('s-dad', { name: 'Dad', phones: [field('Mobile', '+1 555 303 1000')], starred: true }, now),
    card('s-grandma', { name: 'Grandma Rose', phones: [field('Home', '+1 555 222 1961')], birthday: birthdayIn(2, 1939), media: ['video-3'] }, now),
    card('s-leo', { name: 'Leo (best friend)', phones: [field('Mobile', '+1 555 515 0101')], birthday: birthdayIn(30, 2015) }, now),
  ];
}

const FACES = {
  dad: ['RC', '#5eead4', '#2563eb'],
  mom: ['LC', '#f9a8d4', '#a855f7'],
  rose: ['RA', '#fde68a', '#f97316'],
  ana: ['AL', '#a7f3d0', '#059669'],
  'j-alex': ['AC', '#93c5fd', '#4f46e5'],
  'j-dad': ['RC', '#5eead4', '#2563eb'],
  's-mom': ['LC', '#f9a8d4', '#a855f7'],
};

export class DemoContacts {
  books = new Map(); // person -> cards
  history = new Map(); // person -> [{ at, reason, count, cards }]
  photos = new Map(); // `${person}:${id}` -> picture address set during the visit

  book(person) {
    if (!this.books.has(person)) {
      const now = Date.now();
      const make = { alex: alexBook, jordan: jordanBook, sam: samBook }[person];
      const cards = make ? make(now) : [];
      this.books.set(person, cards);
      this.history.set(person, cards.length ? [{ at: now - 3 * DAY, reason: 'Before importing Pixel 9 (contacts.vcf)', count: Math.max(0, cards.length - 4), cards: cards.slice(4).map((c) => ({ ...c })) }] : []);
    }
    return this.books.get(person);
  }

  keep(person, reason) {
    const cards = this.book(person);
    if (!cards.length) return;
    this.history.get(person).unshift({ at: Date.now(), reason, count: cards.length, cards: cards.map((c) => ({ ...c })) });
  }

  async contacts(person) {
    return [...this.book(person)].sort((a, b) => a.name.toLowerCase().localeCompare(b.name.toLowerCase())).map((c) => ({ ...c }));
  }

  async contactSave(person, contact) {
    const cards = this.book(person);
    const tidy = (list) => (list || []).filter((f) => f.value?.trim());
    const next = { ...contact, phones: tidy(contact.phones), emails: tidy(contact.emails), addresses: tidy(contact.addresses), urls: tidy(contact.urls), modified: Date.now() };
    if (!next.name?.trim()) next.name = [next.given, next.family].filter(Boolean).join(' ') || next.nickname || next.org || next.phones[0]?.value || next.emails[0]?.value || '';
    if (!next.name) throw new Error('a contact needs a name, a number or an address');
    const at = cards.findIndex((c) => c.id === next.id);
    if (at >= 0) cards[at] = { ...cards[at], ...next, photo: cards[at].photo, photoAt: cards[at].photoAt };
    else cards.push({ ...next, id: `new-${Date.now()}`, added: Date.now(), source: 'Castle', photo: '', photoAt: 0 });
    return { ...(at >= 0 ? cards[at] : cards[cards.length - 1]) };
  }

  async contactsDelete(person, ids) {
    const cards = this.book(person);
    this.keep(person, `Before deleting ${ids.length} contact${ids.length === 1 ? '' : 's'}`);
    const before = cards.length;
    this.books.set(person, cards.filter((c) => !ids.includes(c.id)));
    return { deleted: before - this.books.get(person).length };
  }

  async contactsImport(person, file) {
    const text = await file.text();
    const cards = text.split(/END:VCARD/i).filter((chunk) => /BEGIN:VCARD/i.test(chunk));
    if (!cards.length) throw new Error('there are no contacts in that file (Castle reads vCard .vcf files)');
    this.keep(person, `Before importing ${file.name}`);
    let added = 0;
    for (const chunk of cards) {
      const name = (chunk.match(/^FN[^:]*:(.*)$/im) || [])[1]?.trim();
      const tel = (chunk.match(/^(?:item\d+\.)?TEL[^:]*:(.*)$/im) || [])[1]?.trim();
      if (!name && !tel) continue;
      this.book(person).push({ ...card(`imp-${Date.now()}-${added}`, { name: name || tel, phones: tel ? [field('', tel)] : [], source: file.name }, Date.now()), added: Date.now() });
      added++;
    }
    return { added, skipped: 0, empty: cards.length - added };
  }

  async contactsExport(person, ids = null) {
    const lines = (await this.contacts(person))
      .filter((c) => !ids || ids.includes(c.id))
      .map((c) => ['BEGIN:VCARD', 'VERSION:3.0', `FN:${c.name}`, `N:${c.family};${c.given};;;`, ...c.phones.map((p) => `TEL:${p.value}`), ...c.emails.map((e) => `EMAIL:${e.value}`), 'END:VCARD'].join('\r\n'));
    return new Blob([lines.join('\r\n') + '\r\n'], { type: 'text/vcard' });
  }

  async contactDuplicates(person) {
    const cards = this.book(person);
    const groups = [];
    const used = new Set();
    for (const a of cards) {
      if (used.has(a.id)) continue;
      const keys = contactKeys(a);
      const same = cards.filter((b) => b !== a && !used.has(b.id) && [...contactKeys(b)].some((k) => keys.has(k)));
      if (!same.length) continue;
      const reasons = new Set();
      for (const b of same) for (const k of contactKeys(b)) if (keys.has(k)) reasons.add({ p: 'same phone', e: 'same email', n: 'same name' }[k[0]]);
      const members = [a, ...same].sort((x, y) => fullness(y) - fullness(x));
      members.forEach((m) => used.add(m.id));
      groups.push({ ids: members.map((m) => m.id), reasons: [...reasons].sort() });
    }
    return groups;
  }

  async contactsMerge(person, ids) {
    const cards = this.book(person);
    const picked = ids.map((id) => cards.find((c) => c.id === id)).filter(Boolean);
    if (picked.length < 2) throw new Error('pick at least two contacts to merge');
    this.keep(person, `Before merging ${picked.map((c) => c.name).join(', ')}`);
    const [keep, ...rest] = picked;
    for (const o of rest) {
      for (const k of ['given', 'family', 'nickname', 'org', 'title', 'birthday']) if (!keep[k]) keep[k] = o[k];
      if (o.notes && !keep.notes.includes(o.notes)) keep.notes = keep.notes ? `${keep.notes}\n\n${o.notes}` : o.notes;
      for (const k of ['phones', 'emails', 'addresses', 'urls']) {
        for (const f of o[k]) if (!keep[k].some((x) => x.value.toLowerCase().replace(/\D/g, '').slice(-10) === f.value.toLowerCase().replace(/\D/g, '').slice(-10) && (k !== 'emails' || x.value.toLowerCase() === f.value.toLowerCase()))) keep[k].push(f);
      }
      keep.groups = [...new Set([...keep.groups, ...o.groups])];
      keep.media = [...new Set([...keep.media, ...o.media])];
      keep.starred ||= o.starred;
      if (!keep.photo && o.photo) Object.assign(keep, { photo: o.photo, photoAt: Date.now() });
    }
    keep.modified = Date.now();
    this.books.set(person, cards.filter((c) => !rest.includes(c)));
    return { ...keep };
  }

  async contactHistory(person) {
    this.book(person);
    return this.history.get(person).map(({ at, reason, count }) => ({ at, reason, count }));
  }

  async contactsRestore(person, at) {
    const copy = this.history.get(person)?.find((h) => h.at === at);
    if (!copy) throw new Error('that copy is gone');
    this.keep(person, 'Before restoring an earlier copy');
    this.books.set(person, copy.cards.map((c) => ({ ...c })));
    return { count: copy.cards.length };
  }

  async contactPhoto(person, id) {
    const set = this.photos.get(`${person}:${id}`);
    if (set !== undefined) return set;
    const c = this.book(person).find((x) => x.id === id);
    if (!c?.photo) return null;
    const [initials, from, to] = FACES[id] || [c.name.slice(0, 2).toUpperCase(), '#cbd5e1', '#475569'];
    return face(initials, from, to);
  }

  async contactPhotoSet(person, id, file) {
    const c = this.book(person).find((x) => x.id === id);
    if (!c) throw new Error('that contact is gone');
    this.photos.set(`${person}:${id}`, file ? URL.createObjectURL(file) : null);
    Object.assign(c, { photo: file ? file.type : '', photoAt: Date.now(), modified: Date.now() });
    return { ...c };
  }
}

function fullness(c) {
  return c.phones.length + c.emails.length + c.addresses.length + (c.photo ? 2 : 0) + (c.notes ? 1 : 0) + (c.birthday ? 1 : 0);
}
