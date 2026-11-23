# Next chapter: AI-aware semantic GUI and intent-aware desktop

**Status: PARKED / DESIGN DIRECTION, NOT AN ACTIVE IMPLEMENTATION MISSION**  
**Recorded:** 2026-11-23 (Matthew + Lucy)  
**Starting point inspected:** `main` at `53b0b96` (merge of 044, dated 2026-11-13).  
**Purpose:** Preserve the direction for the next time Hamesh returns to the workshop rotation. Do not automatically start work, assign agents, or replace current priorities.

## The central idea

Hamesh began as **Eye / Hand / Memory: See. Understand. Act. Prove it worked.**
Today its retained component tree, revisions, compact map/detail, geometry, visible/occluded state, command paths, inspector, CLI/MCP and evidence machinery already give agents structural knowledge of Hamesh applications without screenshot-driven navigation.

The next chapter extends **what the interface means**, not merely how the AI clicks it.

> The GUI describes itself to the OS and AI in a structured, machine-operable way. A user can correct its conclusions through ordinary deliberate actions. The environment learns working methods, helps with relevant information, and stays useful with AI turned off.

Distinguish:
1. **Facts:** a component's identity, type, visibility, contents/state, relationships, offered actions, and observed events, backed by deterministic application state.
2. **Hypotheses:** what a user may be trying to achieve, inferred from activity, choices and context. Always provisional and correctable.
3. **Authority:** what an agent may do. Never inferred simply from state, component visibility, or model confidence.

## Scope split: do not turn Hamesh into the OS

- **Hamesh (framework):** retained semantic component/object model, typed descriptions, relations, capabilities, meaningful events, inspectable visual state, native controls/rendering, accessibility and human/agent parity.
- **OS intelligence service (future):** contextual relevance, attention policy, learned preferences, current user intent, background monitoring, durable conditional intentions and explanations. Model-independent; rules-first when sufficient.
- **Tiling/floating window manager (future):** layout/placement of persistent tiled surfaces, short-lived floating tools and objects attached to a parent; focus-safe contextual presentation. Read semantic hints rather than guess from window titles.
- **Omen / command palette (future):** discover and invoke authorised capabilities; accept explicit intentions such as 'watch this', 'show me only decisions', 'keep this panel beside my editor'.
- **Job infrastructure / Oracle / Switchboard / agents (integrations, not Hamesh dependencies):** produce evidenced status events and execute delegated work only under independently enforced permissions.
- **Pepin and foreign-language applications:** expose these contracts via a language-neutral stable ABI/protocol, with rich, idiomatic wrappers where useful. Hamesh should not be tied to Rust-only authoring.

This is a companion to the existing Hamesh architecture, not a request to rewrite the renderer, replace app state, build a new window manager now, or invent a second agent coordinator.

## Next-level semantic object contract

Extend the Eye's existing `map` (cheap overview) / `detail` (on demand) with optional **typed, versioned, app-owned semantic descriptions**. Do not break existing inspectors or rely on free-form prose as executable truth.

For each meaningful object or widget, expose what is reasonable:
- stable object identity and **kind** (editor, list, job, wave/timeline, chart, 3-D scene, etc.)
- human-friendly title and the **domain entity** it represents (a task, audio take, source file, project)
- parent/child and **domain relationships** (`belongs_to`, `depends_on`, `attached_to`, `produced_by`)
- machine-readable **state** and relevant properties; indicate freshness, provenance and unknowns
- explicitly named, typed **capabilities** with argument shapes, authority requirements, effect semantics and verification receipts
- meaningful **events** and subscriptions/diffs (e.g. task needs review; waveform selection changed), not continuous scene polling
- optional, permission-controlled **sensory inspection** for inherently perceptual content.

Example: an audio waveform can expose project/track/take identity, timeline bounds, selected region, playing/recording state, clip relations and editing operations. For a question about timbre, the AI may additionally request the relevant audio excerpt. For a visual design judgement, it may request a screenshot. Structured inspection is the default, not a prohibition on multimodal perception.

Do not force all domains into one giant universal enum: provide a small interoperable core plus **extensible domain schemas**, with version negotiation and optional fields. Progressive disclosure is essential for large applications and limited model context budgets.

## Human preference as an explicit signal

The user's deliberate choices are **stronger evidence** about their current intention than passive observation, but are not guaranteed evidence of *why* they chose something:
- passive use / focus patterns: weak, contextual signal;
- selecting an alternative, moving/resizing, pinning/attaching/dismissing a panel: stronger signal;
- explicit statements ('keep this beside Pepin', 'not now', 'only show blocked jobs'): strongest signal for the stated scope.

Distinguish **not relevant** from **not now**. A one-off move is not automatically a permanent preference. Learn *relationships* ('terminal beside editor; agent activity secondary, persistent'), not just raw coordinates. Prefer to apply learned layouts at natural transition points (open workspace/add object), not continuously rearrange the screen under the user.

The system must allow inspect/edit/forget/undo of learned rules and must explain why it surfaced something. Never assume that a selection proves a hidden motive. Persistent personal modelling requires clear local storage and user controls.

## Attention and durable intentions (future OS consumers)

An **Awareness Tile** may follow the user across unrelated workspaces, respecting limited screen area and focus. It should distinguish:
- routine information: update silently;
- opportunity: show quietly if contextually useful;
- decision/blocker: persistent, actionable, reason explained;
- genuine critical event: interrupt when warranted.

A dozen completion events can aggregate into a concise state and next action. A job being 'done' is **not** proof it passed verification or was accepted. Each event should identify provenance/evidence and consequences of inaction.

**Persistent intentions** are durable, scoped, inspectable conditions outside chat text:
- 'Watch Pepin's compiler tests and tell me on regressions.'
- 'When Muse's current packet is independently verified, give him the next *pre-approved* packet; otherwise ask me.'
- 'Keep agent activity visible without stealing focus while I work on music.'

A monitoring service evaluates conditions; an authorised executor acts within granted bounds; the user is asked for decisions or when an agent cannot actually be awakened. No implied authority for merges, deletions or spending. The ordinary desktop remains functional offline and without models.

## Tiling/floating spatial contract (future window manager consumer)

Tiling is the default for sustained work. Floating is for short-lived instruments; useful state variants: **temporary**, **attached** (to another object) and **pinned** (persistent). Objects can be promoted into tiles. Prefer semantic layout relationships over saved pixel coordinates; respect user overrides and never steal focus on speculative relevance.

A command palette is the discoverability and command entry point, **not an always-visible application dock**. Everything should be operable without AI.

## Smallest useful experiment when work resumes

**Do not begin with a whole desktop.** Use the existing Hamesh tasks/workbench app and Eye/Hand interfaces.

1. Define a minimal typed `SemanticObject` + `Relation` + `Capability` + `MeaningfulEvent` schema as an **additive, backwards-compatible** extension. Preserve stable IDs, rev stamps, and current `map`/`detail` behaviour. Decide carefully whether to store typed data beside the current `HashMap<String,String>` props rather than replace it.
2. Implement **one realistic task/job object**: belongs to project, assigned to agent, status, last evidence, reason human intervention is required, allowed inspect action. Expose it via compact overview + on-demand detail, and meaningful change notification. Demonstrate external client inspection **without screenshots**.
3. Show a tiny **Awareness Tile demonstration** consuming these events, surfacing one actionable blocker while quietly consolidating successful completions. No new global orchestration daemon required for this slice.
4. Capture a simple human correction ('this belongs with Pepin', 'not now') and demonstrate that a *local, inspectable, reversible* rule changes the next suggestion. No opaque claims that learning always improves.
5. Test a non-Hamesh object as a **schema thought experiment** (audio waveform metadata + optional sensory handoff) before generalising. Do not implement a DAW for this milestone.
6. Use existing Terror Bats and independent review to verify revision-safe state, action/effect receipts, user vs agent routing, bounded memory/context and unchanged input responsiveness.

**Acceptance evidence:** a runnable Hamesh demo; a compact external inspection response; an event change witnessed without screenshot polling; a user correction visibly alters one subsequent suggestion and can be reversed; forbidden/direct actions remain permission-gated; tests and realistic latency receipts tied to a commit. Clearly distinguish demo behavior from OS-wide functionality.

## Deferred follow-ons (not in first slice)

- full tiling/floating window manager, cross-workspace object placement and adaptive layout engine;
- OS-wide durable intent scheduler / agent delegation / remote computer awareness;
- cross-application preference transfer and model-assisted relevance inference;
- waveform, 3-D, map and other rich semantic widget families;
- multimodal inspection providers beyond explicit bounded adapters.

## Read-this-first on return

1. Inspect current `main`, open branches/worktrees, issues, downstream clients, CI and local acceptance. This plan was recorded against `53b0b96`, not a claim about future head.
2. Review `README.md`, `docs/architecture.md`, `docs/inspector-protocol.md`, `crates/hamesh-core`, `crates/hamesh-inspect`, `crates/hamesh-automation`, `crates/hamesh-blocks/catalogue.json`, `crates/hamesh-widgets/src/desktop.rs`, and the 042–044 receipts.
3. Preserve existing Eye / Hand / Memory invariants and compatible language bindings.
4. Agree on the smallest testable schema first, then implement one vertical slice, verify independently and stop to review. Do not let this parked chapter pre-empt active priorities.

**Related:** [[Hamesh — Current Status and Direction]] in Basic Memory; the Workshop software-family inventory; Pepin; Omen; Oracle; the intended AI-first desktop.
