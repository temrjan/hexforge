# hexforge — Design Brief (for the Claude web Design tool)

You are designing the UI for **hexforge**, a small **native desktop application**
(Linux / GNOME). It is a "vanity Ethereum address" generator: the user types a
short hex word and the app searches for an Ethereum address that contains that
word, showing the matching wallet(s).

**This is NOT a website.** The app is built in **Rust with `egui`** (an
*immediate-mode* GUI). Please design within that medium's limits (see
Constraints). Your output is a **visual target + a design-tokens spec** that a
developer will re-implement in egui.

---

## 1. What I need you to deliver

1. **A single self-contained `index.html`** (inline CSS, no external assets) that
   renders the app window in **dark theme** with realistic sample data and shows
   the main states. This is for previewing/screenshotting the look.
2. **A "Design Tokens" section** (a table) listing every color, font, size,
   spacing value, and corner radius. **This is the part that gets implemented**,
   so be explicit and complete.
3. Keep it **clean, modern, minimal** — a focused single-purpose tool, not a
   dashboard. Think "developer utility with taste".

---

## 2. Hard constraints (target = egui, not a browser)

The developer can faithfully reproduce these in egui:
- **Solid fill colors**, one **accent color**, text colors.
- **Rounded corners** (per widget/card).
- **Subtle soft drop-shadow** on cards/panels (one soft shadow, low opacity).
- **Borders / 1px strokes**.
- **Per-state widget colors**: normal / hover / active(pressed) / disabled.
- **Custom fonts** (one UI sans + one monospace) — name real, embeddable fonts
  (e.g. **Inter** for UI, **JetBrains Mono** or **Fira Code** for addresses/keys).
- A simple **spinner** (built-in).
- Scrollable content area.

Please **AVOID** (hard to reproduce in immediate-mode egui — do not rely on them):
- CSS gradients, glassmorphism / backdrop blur, background images/textures.
- Animations / transitions / hover-grow effects.
- Drop-shadow stacks, glows, complex layered effects.
- Icon-heavy UI (icons need a font/SVG pipeline). Prefer **text labels**; at most
  a couple of simple glyphs.
- Flexbox/grid-specific layouts — layout is a **manual vertical stack** of rows.

Window: **resizable, single window**, default ≈ **560 × 460 px**, min ≈ 480 × 420.

---

## 3. The screen — structure, elements, states

Single window, three regions stacked vertically:

### A. Header / controls (fixed at top)
- App title: **`hexforge`** (heading).
- Row: label **`Слово:`** + a single-line **text input** (placeholder/hint:
  `deadbeef`).
- Inline **validation error** (red text) under the input when the word is invalid,
  e.g. `non-hex character 'y' (addresses use only 0-9, a-f)`. (Hidden when valid
  or empty.)
- Row: label **`Где:`** + three **radio buttons**: `в начале` · `везде` · `в конце`
  (start / anywhere / end). One selected.
- Row: three **buttons**: **`Искать`** (primary), **`Стоп`** (secondary),
  **`Очистить`** (secondary). Buttons can be **disabled** (greyed) depending on
  state — show a disabled style.
- **Status line** (changes by state — design all four):
  - *Idle:* `Введите hex-слово и нажмите «Искать».`
  - *Running:* a **spinner** + `1 234 567 ключей · 4500/с · найдено 1/1 · 12с`
  - *Done:* `Готово · найдено 9`
  - *Error:* red `Ошибка: …`

### B. Results list (scrollable, middle — grows/scrolls)
- A small header: `Найдено: 9`.
- A vertical list of **wallet cards**. Each card:
  - The **address** in monospace, e.g. `0x6a6c7f5d9dfb28a7608d55c6372facade5c203a5`
    (the matched substring, e.g. `facade`, may be subtly highlighted — optional).
  - A **`Копировать адрес`** button.
  - A **`Показать секрет`** toggle button (becomes `Скрыть` when open).
  - When revealed, two monospace lines appear inside the card:
    - mnemonic: `reflect spawn enlist book race urban like sibling portion regret abstract rabbit`
    - private key: `0x578bd86deea142e233662206610b4599557711cd40bde52a601fef53c0396f42`
  - Secrets are **hidden by default** (security) — design the revealed and hidden
    states.
- Empty-while-running placeholder: `Пока ничего не найдено — ищем…`

### C. Footer (fixed at bottom)
- Small muted text: `🔒 Offline · ключи не сохраняются на диск · выпиши seed на бумагу`

---

## 4. Tone & functional context (so the mockup feels right)
- It generates **private keys** → a security-conscious, trustworthy feel. Muted,
  serious palette; the accent used sparingly for the primary action and matches.
- Works **offline**, saves **nothing** to disk.
- Brand hint: name = **hex** + **forge**. The current icon is a hexagon outline
  with `0x` in an **amber/orange** accent (`#f5a623`) on near-black. Feel free to
  keep amber as the accent or propose a better one — but give exact hex.

---

## 5. Design Tokens to specify (the deliverable table)

Give exact values for all of these:

**Colors (hex):** window background · card/panel background · card border ·
text primary · text secondary/muted · accent · accent hover · accent pressed ·
on-accent (button text) · danger/error · success · monospace text · scrollbar.

**Typography:** UI font family · monospace font family · sizes for {title,
heading, body, small/footer, monospace address} · weights {regular, medium/bold}.

**Spacing:** base spacing scale (e.g. 4 / 8 / 12 / 16) · card inner padding ·
gap between rows · gap between cards.

**Shape:** corner radius for {window, card, button, input}.

**States (give the color set for each):** button default / hover / pressed /
disabled · input normal / focused / error · card default (+ optional shadow:
offset, blur, color/opacity).

---

## 6. Output, please
1. `index.html` (inline CSS, dark theme) rendering the window with the sample data
   above, showing at least the **Done state with a few result cards** (one card
   revealed, one hidden) + the controls + footer.
2. The **Design Tokens** table from §5.
3. (Optional) a short note on any choice that might be hard in egui, so the
   developer can adapt.

Keep it implementable under §2. Thank you!
