// People across sections: how a conversation in Comms finds its person in Contacts, and
// the other way round. A phone number is its last ten digits (so "+1 (555) 010-2030" and
// "555-010-2030" are one phone), an email address is itself in lower case, and a chat
// handle or a Messenger name matches a card's name or nickname.
//
// Same rules as the box (castled/src/contacts.rs phone_key), so the two never disagree.

/** The part of a phone number that names the line, or '' when it is not a phone number. */
export function phoneKey(number) {
  const digits = String(number || '').replace(/\D/g, '');
  return digits.length < 5 ? '' : digits.slice(-10);
}

const nameKey = (name) => String(name || '').toLowerCase().split(/[^\p{L}\p{N}]+/u).filter(Boolean).join(' ');

/** What a person's card answers to: "p:<phone>", "e:<email>", "n:<name>". */
export function contactKeys(contact) {
  const keys = new Set();
  for (const p of contact.phones || []) {
    const k = phoneKey(p.value);
    if (k) keys.add(`p:${k}`);
  }
  for (const e of contact.emails || []) if (e.value) keys.add(`e:${e.value.trim().toLowerCase()}`);
  for (const n of [contact.name, contact.nickname]) {
    const k = nameKey(n);
    if (k) keys.add(`n:${k}`);
  }
  return keys;
}

/** The keys of one number, address or name, as a conversation lists its people. */
export function addressKey(address) {
  const text = String(address || '').trim();
  if (text.includes('@')) return `e:${text.toLowerCase()}`;
  const phone = phoneKey(text);
  // A short code or a name with a few digits in it is not a phone number.
  if (phone && /^[\d\s()+\-.]+$/.test(text)) return `p:${phone}`;
  const name = nameKey(text);
  return name ? `n:${name}` : '';
}

/**
 * Finds people in an address book by what they answer to.
 * @param {object[]} contacts
 */
export function peopleIndex(contacts) {
  const byKey = new Map();
  for (const c of contacts || []) for (const k of contactKeys(c)) if (!byKey.has(k)) byKey.set(k, c);
  return {
    /** The card for one number, address or name, or null. */
    find(address) {
      const k = addressKey(address);
      return (k && byKey.get(k)) || null;
    },
    /** The cards of a conversation's people, each once, in the conversation's order. */
    forConversation(conversation) {
      const found = [];
      for (const p of conversation?.participants || []) {
        const c = this.find(p);
        if (c && !found.includes(c)) found.push(c);
      }
      // A conversation titled with a name ("Dad") is that person when nothing else matched.
      if (!found.length && conversation?.title) {
        const c = byKey.get(`n:${nameKey(conversation.title)}`);
        if (c) found.push(c);
      }
      return found;
    },
  };
}

/** True when a conversation is with this person (one of its people is them). */
export function isWith(conversation, contact) {
  const keys = contactKeys(contact);
  if ((conversation.participants || []).some((p) => keys.has(addressKey(p)))) return true;
  return keys.has(`n:${nameKey(conversation.title)}`);
}

/** "Dad", or the initials of a name for an avatar: "AM" for "Ana María López". */
export function initials(name) {
  const words = String(name || '').trim().split(/\s+/).filter(Boolean);
  if (!words.length) return '?';
  const pick = words.length === 1 ? [words[0]] : [words[0], words[words.length - 1]];
  return pick.map((w) => [...w][0].toUpperCase()).join('');
}
