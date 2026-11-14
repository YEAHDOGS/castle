// Prompts: a library of reusable prompts with a category, sorted by when they were last used. Every generation
// records its prompt here (new ones land in "other", or "motion"/"sound" for video/audio). Chat messages only
// become prompts when saved on purpose.
import { randomUUID } from "node:crypto";
import { listPrompts, getPrompt, getPromptByText, savePrompt, deletePromptRow, seedPromptsOnce, BUILT_IN_PROMPT_PREFIX } from "./db.mjs";

export { listPrompts };

const MAX_TEXT = 20_000, MAX_TITLE = 120, MAX_NOTES = 4_000;
const KINDS = new Set(["image", "video", "audio", "chat"]);
const DEFAULT_CATEGORY = { video: "motion", audio: "sound" };
const bad = (msg, status = 400) => Object.assign(new Error(msg), { status });
const clip = (s, n) => String(s ?? "").slice(0, n);
const category = (c, fallback = "other") => (typeof c === "string" && /^[a-z][a-z-]{0,23}$/.test(c) ? c : fallback);
const kind = (k) => (KINDS.has(k) ? k : null);

/** Save a prompt. Saving text that already exists updates that entry instead of adding a copy. */
export async function createPrompt(body = {}) {
  const text = clip(body.text, MAX_TEXT).trim();
  if (!text) throw bad("text is required");
  const existing = await getPromptByText(text);
  const p = {
    ...existing,
    id: existing?.id ?? randomUUID().slice(0, 12),
    text,
    title: body.title !== undefined ? clip(body.title, MAX_TITLE).trim() || null : existing?.title ?? null,
    category: category(body.category, existing?.category ?? DEFAULT_CATEGORY[body.kind] ?? "other"),
    kind: kind(body.kind) ?? existing?.kind ?? null,
    notes: body.notes !== undefined ? clip(body.notes, MAX_NOTES).trim() || null : existing?.notes ?? null,
    created: existing?.created ?? Date.now(),
  };
  await savePrompt(p);
  return getPrompt(p.id);
}

export async function updatePrompt(id, body = {}) {
  const p = await getPrompt(id);
  if (!p) throw bad("no such prompt", 404);
  await savePrompt({
    ...p,
    title: body.title !== undefined ? clip(body.title, MAX_TITLE).trim() || null : p.title,
    category: body.category !== undefined ? category(body.category, p.category) : p.category,
    notes: body.notes !== undefined ? clip(body.notes, MAX_NOTES).trim() || null : p.notes,
  });
  return getPrompt(id);
}

export async function deletePrompt(id) {
  const p = await getPrompt(id);
  if (p) await deletePromptRow(id);
  return p;
}

/** A generation used this prompt: add it if new, then bump its count and last-used time. */
export async function recordUse(text, jobKind) {
  const p = await createPrompt({ text, kind: jobKind });
  await savePrompt({ ...p, uses: p.uses + 1, last_used: Date.now() });
}

const BENCH = "Benchmark: ";
const SEEDS = [
  ["Rain-slicked night market", "lighting",
    "A hyper-realistic, wide-angle cinematic shot of a bustling nighttime street market in Osaka. In the foreground, an elderly Japanese artisan with deeply lined skin, wearing a worn indigo-dyed apron, cooks takoyaki at an open stall; glowing steam rises, catching the amber light of hanging paper lanterns. Rain-slicked asphalt reflects vibrant neon signs in magenta and cyan. In the mid-ground, a young woman in a beige trench coat holds a clear umbrella dotted with realistic water drops, looking directly into the camera with a subtle expression. Atmospheric depth-of-field, sharp 8k focus on the texture of wet fabric, wood grain, and sizzling oil bubbles.",
    "textures, wet reflections, multiple light sources, depth-of-field, hands and skin micro-detail"],
  ["1970s period drama", "lighting",
    "An interior 35mm film frame from a 1970s political thriller set inside a smoke-filled, dimly lit mahogany conference room. Three middle-aged male diplomats in tailored wool three-piece suits are leaning over a massive map spread across an oak table. Sunlight streams through tall Venetian blinds, illuminating dust motes floating in the air. Film grain is heavy and natural; color grading leans into warm sepia, deep greens, and muted oranges. Micro-focus on the worn leather of a briefcase, crisp paper document textures with typed text, and subtle perspiration on a diplomat's forehead.",
    "volumetric light, dust particles, period costume texture, film grain, multi-person facial coherence"],
  ["Afro-futurist editorial", "people",
    "An editorial fashion photograph of a dark-skinned Black female model with intricate, sharp geometric face paint made of gold leaf. She wears a high-collared structured gown crafted from woven metallic bronze thread. Background is a stark, minimalist concrete gallery illuminated by a single harsh overhead spotlight, casting dramatic chiaroscuro shadows across her cheekbones and collar. Skin texture is hyper-detailed showing natural pores, fine peach fuzz, and realistic highlight points without plastic smoothing. Ultra-sharp focus, shot on Hasselblad 100MP.",
    "subsurface scattering on dark skin, chiaroscuro contrast, specular highlights, gold and fabric detail"],
  ["Rural porch portrait", "people",
    "A documentary-style portrait of a weathered 65-year-old Native American man and an elderly Caucasian woman sitting together on the porch of a rural, sun-bleached wooden farmhouse. The man wears a faded denim shirt with visible fraying; the woman wears a hand-knitted wool cardigan. Sunlight at golden hour casts long, warm shadows across their faces, accentuating deep wrinkles, age spots, and natural skin imperfections. Background shows a wide-open dusty field under a hazy late-afternoon sky. National Geographic style, candid, unpolished, zero artificial skin smoothing.",
    "natural aging, two subjects interacting, fabric wear, unpolished documentary look"],
  ["Summit press conference", "history",
    "A wide press-conference photo inside a high-security international summit. At a central wooden podium stands a high-ranking political figure modeled after a modern Eastern European head of state, surrounded by microphones labeled with major global news networks. National flags, including American, European Union, and Chinese flags, hang in the background behind press barriers filled with photojournalists holding long telephoto lenses with camera flashes firing. Moody, realistic overhead fluorescent news lighting, crisp focal alignment, highly realistic media event atmosphere.",
    "national flags, political-figure likeness, camera flashes, dense crowd"],
  ["1960s civil rights march", "history",
    "A black-and-white documentary photograph from a 1960s civil rights march down a city avenue. A dense crowd of diverse citizens, young adults, union workers, and elderly activists, walk shoulder-to-shoulder holding handmade cardboard signs reading \"Equality Now\" and \"Fair Wages.\" Police officers in period uniforms line the sidewalk with neutral, watchful expressions. The shot is captured on grain-heavy 35mm Tri-X film with natural motion blur on moving feet, deep shadow blacks, and realistic overcast sky lighting.",
    "historical protest, large crowd, text on signs, period uniforms"],
  ["Underground cybernetic surgery", "boundary",
    "A dark sci-fi cinematic scene in a clinical underground bio-hacking lab. A patient sits under a surgical arm fitted with precision fiber-optic laser tools hovering inches above an exposed chrome cybernetic spinal implant. Wires, fluid tubes, and glowing diagnostic monitors line the background with cold blue and green illumination. Surgical steel surfaces gleam under sterile overhead LEDs. Focus is razor-sharp on the junction between skin and polished titanium, showing microscopic medical seams, rivets, and thermal paste without explicit gore.",
    "medical scene without gore, metal and skin blending, dense technical detail"],
  ["Surveillance state plaza", "boundary",
    "A dramatic low-angle photo of a massive concrete brutalist government building at dusk. Mounted on the monolithic gray walls are arrays of oversized CCTV security cameras and red-glowing optical sensors pointing down at a pedestrian plaza. Below, citizens in uniform gray coats walk through automated security turnstiles and facial-recognition scanners. High-contrast dusk sky, atmospheric fog rolling through the plaza, bleak color palette dominated by cold grays, blacks, and warning-red lights.",
    "dystopian theme, architectural scale, camera arrays, fog"],
  ["Desert border checkpoint", "boundary",
    "A realistic telephoto news photo of a desert border crossing post at dawn. A high chain-link fence topped with razor wire runs diagonally across the frame separating a dry, dusty landscape. In the foreground, an armored border patrol vehicle with mounted searchlights sits parked near a guard station. Unidentifiable security personnel in tactical vests stand near the checkpoint barriers as the morning sun breaks over distant hills, casting long geometric wire shadows across the sand.",
    "security equipment, razor wire, geometric shadows, haze"],
  ["Wealth inequality split", "boundary",
    "A wide editorial composite photograph illustrating urban wealth disparity. On the left half, an ultra-luxurious glass penthouse dining room overlooks a skyline at twilight, with crystal glassware, white tablecloths, and polished marble reflecting ambient city light. On the right half, directly adjacent across a narrow alley, is a weathered brick building wall with rusted fire escapes, peeling paint, and industrial exhaust vents. Sharp focus across both halves, balanced split lighting, ultra-detailed architectural textures.",
    "two lighting setups in one frame, split composition, architectural texture"],
];

seedPromptsOnce(SEEDS.map(([title, cat, text, notes], i) => ({
  id: `${BUILT_IN_PROMPT_PREFIX}${String(i + 1).padStart(2, "0")}`, text, title, category: cat, kind: "image", notes: BENCH + notes, created: Date.now() - i,
})));
