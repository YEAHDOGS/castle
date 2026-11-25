// The demo's Links: pages each demo person "saved", in the shapes the box answers
// (castled/src/links.rs). The sites and people are made up. Saving a link in the demo
// keeps the address only: nothing here reaches the internet, so there is no page to read.

const HOUR = 60 * 60 * 1000;
const DAY = 24 * HOUR;
const SNIPPET_BEFORE = 60;
const SNIPPET_AFTER = 140;

/** A drawn picture standing in for a page's own: two colours and a word. */
function picture(from, to, label) {
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360" viewBox="0 0 640 360">
<defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="${from}"/><stop offset="1" stop-color="${to}"/></linearGradient></defs>
<rect width="640" height="360" fill="url(#g)"/><circle cx="500" cy="110" r="70" fill="rgba(255,255,255,0.14)"/>
<text x="24" y="332" font-family="sans-serif" font-size="22" fill="rgba(255,255,255,0.8)">${label}</text></svg>`;
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

const base = { author: '', description: '', kind: 'page', problem: '', note: '', tags: [], recipe: null, related: [], text: '', art: null };

function alexLinks(now) {
  return [
    {
      ...base, id: 'a1', kind: 'video', url: 'https://www.youtube.com/watch?v=demo0000001', title: 'The butternut pie that beats pumpkin', site: 'YouTube', author: 'Field Kitchen',
      description: 'Roasted butternut squash makes a silkier pie than pumpkin ever will.\n\nFull recipe: https://fieldkitchen.example/butternut-pie\nThe pie dish I use: https://shop.example/pie-dish',
      added: now - 2 * HOUR, note: 'For Thanksgiving. Make two.', tags: ['thanksgiving', 'dessert'], art: picture('#b45309', '#3b1d06', 'BUTTERNUT PIE'),
      related: ['https://fieldkitchen.example/butternut-pie', 'https://shop.example/pie-dish'],
      text: 'Roasted butternut squash makes a silkier pie than pumpkin ever will.\nFull recipe: https://fieldkitchen.example/butternut-pie',
      recipe: {
        name: 'Butternut Squash Pie', source: 'https://fieldkitchen.example/butternut-pie', yields: '8 slices', prep: '25 min', cook: '1 h', total: '1 h 25 min',
        ingredients: ['1 butternut squash (about 2 lb), halved and seeded', '¾ cup brown sugar', '3 eggs', '1 cup evaporated milk', '1 tsp cinnamon', '½ tsp ginger', '¼ tsp nutmeg', '½ tsp salt', '1 unbaked 9-inch pie crust'],
        steps: ['Roast the squash cut side down at 400°F until soft, about 45 minutes. Cool, then scoop out 2 cups.', 'Blend the squash with the sugar, eggs, milk, spices and salt until smooth.', 'Pour into the crust and bake at 350°F for 50 to 55 minutes, until the centre barely wobbles.', 'Cool completely before slicing. It is better the next day.'],
      },
    },
    {
      ...base, id: 'a2', kind: 'recipe', url: 'https://hearthandpan.example/no-knead-bread', title: 'No-Knead Bread, the Overnight Way', site: 'Hearth & Pan', author: 'Mara Quint',
      description: 'Four ingredients, a heavy pot and twelve hours of doing nothing.', added: now - 3 * DAY, tags: ['bread'], art: picture('#a16207', '#422006', 'NO-KNEAD BREAD'),
      text: 'Four ingredients, a heavy pot and twelve hours of doing nothing.\nThe long rest does the kneading for you: the dough is wet enough that the gluten lines up by itself.',
      recipe: {
        name: 'No-Knead Bread', source: '', yields: '1 loaf', prep: '10 min', cook: '45 min', total: '13 h',
        ingredients: ['3 cups bread flour', '1½ tsp salt', '¼ tsp instant yeast', '1½ cups water'],
        steps: ['Stir everything together until no dry flour is left. Cover and leave 12 hours.', 'Fold the dough over itself, shape it into a ball and rest it 1 hour.', 'Heat a lidded pot at 450°F for 30 minutes. Bake the loaf in it 30 minutes covered, then 15 uncovered.'],
      },
    },
    {
      ...base, id: 'a3', url: 'https://homelabnotes.example/quiet-nas', title: 'Building a NAS you cannot hear', site: 'Homelab Notes', author: 'J. Okafor',
      description: 'Big slow fans, drives on rubber, and a case with no window.', added: now - 6 * DAY, tags: ['castle', 'hardware'], art: picture('#0e7490', '#082f49', 'QUIET NAS'),
      text: 'Big slow fans, drives on rubber, and a case with no window.\nMost of the noise from a home server is not the fans. It is the drives, passed into the case and from the case into the shelf it sits on.\nMount each drive on rubber grommets and the hum drops by half.',
    },
    {
      ...base, id: 'a4', url: 'https://trailledger.example/olympic-coast', title: 'Three days on the Olympic coast', site: 'Trail Ledger',
      description: 'Tide tables matter more than the map.', added: now - 20 * DAY, note: 'Ask Jordan about July.', tags: ['trips'], art: picture('#166534', '#052e16', 'OLYMPIC COAST'),
      text: 'Tide tables matter more than the map.\nTwo headlands on this route can only be passed below a four foot tide. Miss the window and you wait six hours on the beach.',
    },
    {
      ...base, id: 'a5', url: 'https://oldforum.example/thread/88213', title: 'https://oldforum.example/thread/88213', site: 'oldforum.example',
      added: now - 41 * DAY, problem: 'oldforum.example says the page is gone (404)',
    },
  ];
}

function jordanLinks(now) {
  return [
    {
      ...base, id: 'j1', url: 'https://greenrow.example/tomatoes-from-seed', title: 'Tomatoes from seed, week by week', site: 'Green Row',
      description: 'Start eight weeks before the last frost, and not a day earlier.', added: now - 1 * DAY, tags: ['garden'], art: picture('#b91c1c', '#450a0a', 'TOMATOES'),
      text: 'Start eight weeks before the last frost, and not a day earlier.\nSeedlings started too soon outgrow their pots and sulk when they are finally planted out.',
    },
  ];
}

const card = (link, snippet = '') => ({
  id: link.id, url: link.url, title: link.title, site: link.site, author: link.author, description: link.description.slice(0, 400), kind: link.kind,
  hasPicture: !!link.art, hasRecipe: !!link.recipe, added: link.added, problem: link.problem, note: link.note, tags: [...link.tags], snippet,
});

function snippetOf(link, words) {
  const sources = [...(link.recipe ? [...link.recipe.ingredients, ...link.recipe.steps] : []), link.text, link.description];
  for (const source of sources) {
    const lower = source.toLowerCase();
    const at = words.map((w) => lower.indexOf(w)).find((i) => i >= 0);
    if (at == null) continue;
    const start = Math.max(0, at - SNIPPET_BEFORE);
    const end = Math.min(source.length, at + SNIPPET_AFTER);
    return `${start > 0 ? '…' : ''}${source.slice(start, end).replace(/\s+/g, ' ')}${end < source.length ? '…' : ''}`;
  }
  return '';
}

export class DemoLinks {
  shelves = new Map(); // person -> links

  of(person) {
    if (!this.shelves.has(person)) {
      const make = { alex: alexLinks, jordan: jordanLinks }[person];
      this.shelves.set(person, make ? make(Date.now()) : []);
    }
    return this.shelves.get(person);
  }

  find(person, id) {
    const link = this.of(person).find((l) => l.id === id);
    if (!link) throw new Error('no such link');
    return link;
  }

  async links(person, q = '') {
    const words = q.toLowerCase().split(/\s+/).filter(Boolean);
    const newest = [...this.of(person)].sort((a, b) => b.added - a.added);
    if (!words.length) return newest.map((l) => card(l));
    const heading = (l) => `${l.title} ${l.site} ${l.author} ${l.tags.join(' ')} ${l.note}`.toLowerCase();
    const body = (l) => `${l.url} ${l.description} ${l.recipe ? `${l.recipe.name} ${l.recipe.ingredients.join(' ')} ${l.recipe.steps.join(' ')}` : ''} ${l.text}`.toLowerCase();
    const found = newest
      .map((l) => ({ l, named: words.every((w) => heading(l).includes(w)), any: words.every((w) => heading(l).includes(w) || body(l).includes(w)) }))
      .filter((hit) => hit.any);
    return [...found.filter((hit) => hit.named), ...found.filter((hit) => !hit.named)].map((hit) => card(hit.l, snippetOf(hit.l, words)));
  }

  async link(person, id) {
    const { art: _art, ...link } = this.find(person, id);
    return structuredClone({ ...link, picture: _art ? 'demo' : '' });
  }

  async linkPicture(person, id) {
    return this.find(person, id).art;
  }

  async linkAdd(person, url, note = '') {
    const address = String(url).split(/\s+/).find((word) => /^https?:\/\//i.test(word)) || (/^\S+\.\S+$/.test(String(url).trim()) ? `https://${String(url).trim()}` : null);
    if (!address) throw new Error('that does not look like a web address');
    const known = this.of(person).find((l) => l.url === address);
    if (known) return this.link(person, known.id);
    let site = address;
    try {
      site = new URL(address).hostname.replace(/^www\./, '');
    } catch {
      throw new Error('that does not look like a web address');
    }
    const link = { ...base, id: `d${Date.now().toString(36)}`, url: address, title: address, site, added: Date.now(), note, problem: 'The demo does not reach the internet, so this page was not read. A box reads it.' };
    this.of(person).push(link);
    return this.link(person, link.id);
  }

  async linkRefresh(person, id) {
    return this.link(person, id);
  }

  async linkUpdate(person, id, changes) {
    const link = this.find(person, id);
    if (changes.title?.trim()) link.title = changes.title.trim();
    if (changes.note != null) link.note = changes.note;
    if (changes.tags) link.tags = [...new Set(changes.tags.map((t) => t.trim().replace(/^#/, '').toLowerCase()).filter(Boolean))];
    return this.link(person, id);
  }

  async linkDelete(person, id) {
    const shelf = this.of(person);
    const at = shelf.findIndex((l) => l.id === id);
    if (at >= 0) shelf.splice(at, 1);
  }
}
