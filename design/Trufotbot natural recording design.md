<!--
Copyright (C) 2026 Ohad Lutzky <lutzky@gmail.com>

SPDX-License-Identifier: GPL-3.0-only
-->

<!-- cSpell: disable -->

# Design: Natural input

## 1. Goal

In a Telegram group that has **exactly one patient**, a message like

    Nurofen kids 7.5

records a dose, with no slash command and no web UI. The same dose must be
recorded by all of these:

| # | Input                 | What makes it tricky                           |
|---|-----------------------|------------------------------------------------|
| 1 | `Nurofen kids 7.5`    | Base case                                      |
| 2 | `Nurofen kids 7.5ml`  | Unit suffix, no space                          |
| 3 | `Nurofen kids 7.5 ml` | Unit suffix, with space                        |
| 4 | `Nurfen kidss 7.5ml`  | Typos, so it needs fuzzy matching              |
| 5 | `Nurofen 7.5ml`       | Another medication is "Nurofen"; disambiguate  |
| 6 | `נורופן 7.5`          | Hebrew brand name (right-to-left); needs alias |

The unit may also be in another script, e.g. `נורופן 7.5 מ"ל` (`מ"ל` is "ml",
and contains a `"`). Like any unit, it is parsed and ignored.

### Non-goals

- Private chats, and groups with zero or several patients, where the patient
  cannot be inferred (§5.1).
- Unit conversion or validation. Any trailing unit is parsed and ignored (§6.2).
- Quantity confirmation prompts. Editing or deleting a recorded dose from its
  notification button is easy, so we accept the risk.
- Backdating (`@10:00`) and `by <user>`.
- Addressing the bot by `@`-mention (§5.1.1).
- Hebrew vowel points (niqqud), final-letter folding, and attached grammatical
  prefixes. The edit-distance budget (§4.3) covers most typos without
  language-specific morphology rules.

## 2. Decision 1: disambiguating "Nurofen" from "Nurofen Kids"

### 2.1 Options considered

- **Option A — Implicit from dose history** (a medication is a candidate if the
  patient has ever taken it): **Rejected.** Suffers from a cold-start problem
  for new medications, keeps a medication eligible forever after a single old
  dose (e.g. when a child outgrows an infant formulation), and gives the user
  no visibility into why a medication was chosen.
- **Option B — Explicit per-patient, per-medication flag** (`natural_input`
  boolean per `(patient, medication)` pair, edited in the UI): **Chosen.**
  Predictable and inspectable. Switching a child from infant to kids to adult
  formulations is a matter of unchecking one box and checking another without
  touching historical doses.
- **Option C — Tri-state hybrid** (Option B with Option A as the default):
  **Rejected.** Requires explaining and testing two mechanisms at once.
- **Option D — Per-patient aliases** (aliasing "nurofen" to "Nurofen Kids" for
  one patient): **Rejected.** Requires a separate per-patient alias store when
  aliases are naturally a property of the medication itself.
- **Option E — Recency tie-breaking** (silently picking whichever medication
  was taken most recently): **Rejected.** Risks silently recording the wrong
  medication.

### 2.2 How disambiguation works

1. **Only medications flagged `natural_input` for that patient are
   candidates.** The flag defaults to **off** and is never turned on
   automatically by recording a dose or running a database migration.
2. **Existing histories are enabled only on explicit request** (§2.4) via the
   `/natural_from_history` Telegram command, which scans recent doses once and
   reports what it enabled. Upgrading therefore leaves bot behaviour unchanged
   until the user opts in.
3. **Scoring prefers the more specific match** (§4.3). If both "Nurofen" and
   "Nurofen Kids" are enabled for a patient, `nurofen 7.5` matches "Nurofen"
   (zero unmatched label tokens), while `nurofen kids 7.5` matches "Nurofen
   Kids". If only "Nurofen Kids" is enabled, `nurofen 7.5` matches it because
   the query tokens are a subset of the label's tokens.
4. **Exact label collisions between enabled medications are rejected at save
   time** (§2.3), preventing a patient from having two enabled medications that
   share a name or alias.
5. **Remaining fuzzy ties ask the user to rephrase** instead of guessing (§5.4),
   such as when both "Nurofen Kids" and "Nurofen Junior" are enabled and the
   user writes `nurofen 5`.

### 2.3 Collision rules

A **label** is a medication's canonical name or one of its aliases. Two labels
have the same **collision key** if they produce the same sorted set of tokens
after normalisation (§4.1). Sorting tokens ensures that `kids nurofen` and
`nurofen kids` are recognised as colliding because the matcher ignores word
order.

Two medications **collide for a patient** if both are enabled for that patient
and any label of one has the same collision key as any label of the other.

The backend enforces this symmetrically whenever a medication is updated:

- **When a medication's name or aliases change:** for every patient who has
  that medication enabled for natural input, its new labels must not collide
  with any other medication enabled for that same patient.
- **When `natural_input` is enabled for a `(patient, medication)` pair:** the
  medication's labels must not collide with any medication already enabled for
  that patient.
- **When `natural_input` is disabled:** no collision check is needed; disabled
  medications may share labels freely.

Why this is enforced in the database transaction after applying the writes:

- Running the check inside the update transaction *after* writing the new name,
  aliases, and flag means a single query naturally sees the post-update state
  (including a simultaneous rename and enable) and rolls the transaction back
  with HTTP `400` if any collision is found.
- Scoping the query to medications that share at least one enabled patient with
  the updated medication avoids false rejections between medications enabled
  only for different patients.
- Only exact collision-key matches are rejected; fuzzy near-neighbours remain
  allowed so legitimate pairs like "Nurofen" and "Nurofen Kids" can coexist for
  the same patient.
- The server is the sole authority because concurrent edits in another tab or
  via Telegram could invalidate any client-side check.

### 2.4 Bulk-enabling from history (`/natural_from_history`)

Rather than backfilling `natural_input` silently during a database upgrade, we
provide `/natural_from_history [patient] [days]` in Telegram (defaulting to a
90-day window):

- **Why Telegram-only:** users discover natural input in Telegram (via `/help`),
  and a chat command can immediately print which medications were enabled and
  which were skipped. The core logic lives in the model layer so an HTTP
  endpoint can be added later if needed.
- **Eligibility:** any medication with at least one recorded dose of
  `quantity > 0` for that patient within the last `days` days (ignoring
  `quantity = 0` "skipped dose" markers) is considered, ordered from most
  recently dosed to least recently dosed.
- **Collision handling:** if a candidate medication collides with an
  already-enabled medication (including one enabled earlier in the same scan),
  it is **skipped and reported** in the reply rather than aborting the command.
  Processing most-recently-dosed first makes the winner deterministic and
  favours the formulation the patient currently uses.
- **Safety:** the operation only turns flags on, preserves existing reminder
  schedules, and is idempotent.

## 3. Decision 2: aliases storage and validation

Aliases are a property of a medication (e.g. a brand name in another language
or a common abbreviation), while enabling natural input is per-patient.

- **Database representation:** SQLite has no array type, so `medications.aliases`
  is stored as newline-delimited text (defaulting to `''`), matching the
  existing convention used for cron schedules.
- **API and UI representation:** the HTTP API exchanges `aliases` as a list of
  strings, and the web UI edits them as a single comma-separated text field
  (§7).
- **Save-time validation:**
  - Any alias containing `,`, `\n`, or `\r` is rejected with `400` so that
    newline storage in SQLite and comma separation in the UI remain
    unambiguous.
  - Whitespace is trimmed, empty entries and duplicates (by collision key) are
    dropped, and any alias matching the medication's own name is stripped.
  - Each medication is capped at 20 aliases, and each alias must satisfy the
    same length and token bounds as a parsed medication name (at least one
    letter, at most 6 tokens and 60 characters).

## 4. Matching algorithm

The matcher in `src/natural_input.rs` is pure (taking a query string and a
slice of candidate items and returning an outcome enum), keeping it independent
of SQLite and Telegram.

### 4.1 Normalisation

Applied to both the user's query and every candidate label:

1. **Strip Unicode bidirectional control characters** (LRM/RLM, embeddings, and
   isolates). Right-to-left text pasted or mixed with digits often contains
   invisible direction marks that are not whitespace and would otherwise stay
   glued to adjacent words or numbers.
2. **Apply Unicode NFKC normalisation** to fold compatibility characters such
   as full-width digits and Hebrew presentation forms.
3. **Lowercase** (a no-op for caseless scripts such as Hebrew).
4. **Replace punctuation with spaces** (including ASCII punctuation and Hebrew
   geresh/gershayim `׳`/`״`) and split on whitespace into tokens. Punctuation
   replacement runs *after* extracting the numeric quantity so decimal points
   survive.

### 4.2 Parsing `<name> <quantity>[unit]`

After stripping bidi marks and applying NFKC:

- The message is split into a leading medication name, a numeric quantity, and
  an optional short trailing unit.
- Whitespace before the quantity is optional (`נורופן7.5` is accepted).
- Only ASCII digits are matched, with either `.` or `,` as the decimal
  separator, and leading-decimal quantities such as `.5` or `,5` are accepted
  as `0.5`.
- The trailing unit (up to 8 non-space, non-digit characters, including `"` or
  `״` for abbreviations like `מ"ל`) is ignored after validation.
- **False-positive guards (rejected as ordinary chatter):**
  - The name part must contain at least one letter and have at most 6 tokens
    and 60 characters.
  - The quantity must be finite and strictly positive (`0` is reserved for
    skipped-dose buttons).
  - If a trailing unit is present, it must contain at least one letter, so a
    question ending in a number and `?` (e.g. `fever 39?`) is treated as
    chatter rather than a dose attempt.

### 4.3 Scoring a label

**Why Damerau-Levenshtein instead of Jaro-Winkler?** `autocomplete.rs` uses
Jaro-Winkler, which biases toward shared prefixes as a user types the beginning
of a word. In natural input, the user types a complete name and a typo can
appear at any position (`Nurfen`, `kidss`). Damerau-Levenshtein treats
insertions, deletions, substitutions, and adjacent transpositions uniformly
anywhere in the token and lets us enforce a discrete per-token edit budget
based on character count (`chars()`, not UTF-8 byte length):

| Token length (chars) | Allowed edits |
|---|---|
| 1–3 | 0 |
| 4–6 | 1 |
| ≥ 7 | 2 |

- Requiring zero edits on 1–3 character tokens prevents short words and short
  brand names from matching unrelated tokens.
- **Every query token must match a distinct token in the label** within its
  edit budget. If any query token has no match, the label is rejected — this is
  the main guard against ordinary sentences that happen to end in a number
  (e.g. `ok see you at 5` or `Nurofen at 5`, where `at` matches nothing).
- Matching labels are ranked lexicographically by
  `(total_edits, unmatched_label_tokens)` (lower is better), and a medication's
  score is the best score across its name and aliases.
- Token order does not affect the score.

### 4.4 Resolution outcomes

Matching a message produces one of five outcomes:

- **`NotADose`:** the text did not parse as `<name> <quantity>[unit]`.
- **`Match`:** a single candidate achieved the best score.
- **`Ambiguous`:** two or more enabled candidates tied for the best score.
- **`NotEnabled`:** no enabled medication matched, but running a second pass of
  the matcher over the patient's *disabled* medications found a match.
- **`NoMatch`:** the text parsed as a dose attempt, but neither enabled nor
  disabled medications matched.

## 5. Backend and Telegram design

### 5.1 Telegram message flow

When a message arrives that is not a recognised slash command:

1. Ignore non-text messages.
2. Ignore replies to other messages (unless the reply text starts with `/`, so
   a mistyped slash command in a reply still gets a 🤷 reaction).
3. If the text starts with `/` (an unrecognised command), react with 🤷 and
   stop.
4. Check whether the sender is an authorised user; log unauthorised senders at
   `debug` and return silently so group chatter from non-authorised members
   does not pollute error logs.
5. In a **private chat**, reply `"Please see /help"` and stop, since the
   patient cannot be inferred.
6. In a **group or supergroup**, look up the patients assigned to the group.
   If the group does not have **exactly one** patient, return silently.
7. Run the matcher for that patient and act on the outcome:
   - **`Match`:** record the dose (sharing the recording helper with `/record`
     so the standard dose notification with **Edit…** / **Repeat** buttons is
     sent) and react 👍 on the user's message.
   - **`Ambiguous`:** reply in plain text listing the tied candidates (§5.4).
   - **`NotEnabled`:** reply explaining that nothing enabled for the patient
     matched, and name the disabled medication(s) that would have matched.
   - **`NotADose`:** do nothing (ordinary group chatter).
   - **`NoMatch`:** react 🤷 on the user's message.

#### 5.1.1 Why `/help` instead of `@`-mentions

We considered having `@trufotbot <message>` reply with diagnostic details about
how a message was parsed, and rejected it:

- Telegram reports mention spans as UTF-16 code-unit offsets; converting those
  to UTF-8 byte offsets to strip the mention is error-prone around multi-byte
  scripts and emoji.
- `/help` is already familiar to users. In a single-patient group, `/help`
  lists the medications enabled for natural input along with their aliases (or
  points to `/natural_from_history` when none are enabled), bounded and
  HTML-escaped to fit within Telegram's 4,096-character message limit.

### 5.2 Group message visibility

By default, Telegram bots in groups only receive slash commands, `@`-mentions,
and replies (*privacy mode*). To read ordinary group messages for natural
input, users can either make the bot an **administrator** of the group
(recommended, as it is scoped to that group) or disable privacy mode globally
in `@BotFather`.

### 5.3 Schema design

The existing `reminders` table already had `(patient_id, medication_id)` as its
primary key, which is the exact key needed for per-patient medication settings.
Rather than introducing a second join table, we renamed `reminders` to
`patients_medications` (and `cron_schedule` to `reminder_cron_schedule`) in one
migration, and added `medications.aliases` and
`patients_medications.natural_input` (defaulting to `0`) in a second migration.

- **No down migrations:** migrations are embedded into the binary and
  downgrading is not supported; documentation advises backing up the SQLite
  database before upgrading.
- **Non-clobbering upserts:** a missing row in `patients_medications` means
  `natural_input = false` and no reminders. Reminder updates and `natural_input`
  updates each use `ON CONFLICT (patient_id, medication_id) DO UPDATE` touching
  only their own column (and supplying `''` for `reminder_cron_schedule` on
  insert), so updating one never overwrites the other.
- **Explicit API fields:** in the HTTP API, `aliases` on medication payloads is
  required (no default-to-empty on deserialization) so an outdated client
  cannot accidentally wipe existing aliases, whereas `natural_input` on update
  requests is optional (`None` leaves the flag unchanged) so a client that
  omits it does not silently disable natural input.

### 5.4 Ambiguity reply

When multiple enabled medications tie, the bot replies in plain text naming the
candidates (including the matched alias in parentheses when matched via an
alias) and asks the user to rephrase. We intentionally avoid inline callback
buttons on ambiguity replies to keep callback state simple and avoid Telegram's
64-byte `callback_data` limit.

## 6. Risks and mitigations

### 6.1 False positives from chatter

In a family group, ordinary messages can end in a number. False positives are
mitigated by layering:

- Only medications explicitly enabled for that patient are candidates.
- Every query token must match a distinct label token (§4.3), and 1–3 character
  tokens allow zero edits.
- The parser caps the medication name at 6 tokens and 60 characters and
  requires any trailing unit to contain a letter (§4.2).
- Replies, non-text messages, and slash commands are ignored.
- Every recorded dose posts a notification with an **Edit…** button (which also
  supports deletion), making undo a single tap.

### 6.2 Units are ignored

The database has no unit column; a quantity always means "whatever unit this
medication is tracked in". Typing `Nurofen 200 mg` for a medication tracked in
mL records `200`. We accept this risk because the dose notification immediately
displays the recorded quantity and provides an **Edit…** button.

### 6.3 Fuzziness choosing the wrong medication

Edit budgets scale with token length and 1–3 character tokens require an exact
match, preventing short brand names from drifting onto unrelated words. When
two enabled medications have near-identical labels that tie on score, the bot
asks the user to rephrase rather than guessing (§5.4).

## 7. Frontend design

- **Comma-separated aliases editor:** aliases are edited in a single text input
  with `dir="auto"` so right-to-left aliases render naturally. While the input
  is focused, the raw text is preserved so typing a trailing comma does not get
  stripped by immediate re-formatting; on blur, the field canonicalises to a
  clean comma-separated list.
- **Per-patient toggle and badge:** the medication detail form provides a
  per-patient checkbox to enable or disable natural input, and the patient
  settings table shows a `natural` badge next to enabled medications.
- **Server-driven collision errors:** if saving fails with HTTP `400` due to a
  label collision, the form displays the server's message verbatim while
  keeping the user's unsaved input intact so they can adjust the aliases or
  uncheck the toggle.

## 8. Summary of key decisions

- **Candidate selection:** explicit per-patient, per-medication `natural_input`
  flag (defaulting to off), never auto-enabled on dose recording or database
  upgrade (§2.1, §2.2).
- **Onboarding existing histories:** explicit `/natural_from_history [patient]
  [days]` Telegram command (90-day default window); collisions are skipped and
  reported with the most recently dosed medication winning (§2.4).
- **Storage:** `reminders` renamed to `patients_medications` for per-patient
  settings; `medications.aliases` stored as newline-delimited text in SQLite,
  `Vec<String>` in the API, and comma-separated text in the UI (§3, §5.3, §7).
- **Collisions:** denied with HTTP `400` inside the update transaction after
  writes whenever two medications enabled for the same patient share a
  normalised token-sorted label (§2.3).
- **Matcher:** pure, DB-free token matcher using Unicode NFKC + bidi stripping
  and per-token Damerau-Levenshtein edit budgets (0 edits for 1–3 chars, 1 for
  4–6, 2 for ≥7), requiring every query token to match (§4).
- **Group and chat behaviour:** natural input runs only in groups assigned to
  exactly one patient; private chats reply `"Please see /help"`; ordinary
  chatter (`NotADose`) is silent; disabled matches get a `NotEnabled` hint;
  ties get a plain-text rephrase prompt; unmatched dose attempts (`NoMatch`)
  and unknown `/commands` get a 🤷 reaction (§4.4, §5.1, §5.4).
