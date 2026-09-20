# Authoring situated recall scenarios

Assertions pass or fail when a scenario runs; measures are only reported.

To author a scenario, copy a narrative with the desired event shape, change IDs and story together, assign exact embedding inputs, then choose assertions that isolate the intended cues. Keep deadlines clearly before or after probes, and ensure every asserted memory already exists. Add an ordinary nearby experience without a forced expectation when testing salience. The loader checks structural consistency; review the story's cue eligibility separately.

This crate runs scripted experiences, authored derived memories and recall probes against Character Memory. A scenario describes what the character experienced and what a caller perceives at recall time; assertions and measurement labels describe the author's expectations. The adapter receives only the input projection, never those expectations.

Use [situated_v1.toml](fixtures/situated_v1.toml) for narrative examples and [situated_loud_topic_v1.json](fixtures/situated_loud_topic_v1.json) for the generated candidate-pressure example. Unsupported capabilities produce an explicit `not_run` outcome for the whole scenario before any namespace is created. A loaded fixture is not evidence that its assertions have executed.

This minimal TOML scenario records a visit, derives a relationship note, then asks for that note when Jo returns:

```toml
schema_version = 3
seed = 7

[[scenarios]]
fixture_id = "jo-returns"
namespace = "jo-returns"
pattern = "situated"
catalog_situations = ["D4"]
character_entity = "mara"
entities = [
  {external_id = "mara", label = "Mara", entity_type = "person", is_hub = false},
  {external_id = "jo", label = "Jo", entity_type = "person", is_hub = false},
]
[scenarios.embedding]
provider = "controllable_similarity"
own_concept = true
seed = 7
vector_size = 16
noise_magnitude = 0.01
clusters = {}
concepts = {}
[scenarios.scenes.pair]
who = [{reference = {by = "key", key = "mara"}}, {reference = {by = "key", key = "jo"}}]

[[scenarios.events]]
kind = "experience"
event_id = "visit"
timestamp = "2025-02-01T10:00:00Z"
text = "Jo and I enjoyed exchanging seeds for our gardens."
scene = {kind = "named", name = "pair"}
[[scenarios.events]]
kind = "derive"
event_id = "gardening-friends"
timestamp = "2025-02-01T10:01:00Z"
memory = {subtype = "relationship_note", text = "Jo and I enjoy sharing seeds.", experiences = ["visit"], about = ["mara", "jo"]}
[[scenarios.events]]
kind = "probe"
event_id = "return"
query_id = "jo-returns-pair"
timestamp = "2025-02-08T10:00:00Z"
scene = {kind = "named", name = "pair"}
[scenarios.events.assertions]
carried = [{memory = "gardening-friends", reason = "pair"}]
cued = [{memory = "gardening-friends", cue = "pair"}]
```

## Fixture and scene shape

The extension selects TOML or JSON; both use schema version 3 and the same admission rules. The root requires `schema_version`, `seed` and a nonempty `scenarios` array. Each scenario requires a unique nonblank `fixture_id` and `namespace`, `pattern = "situated"`, nonempty distinct `catalog_situations`, a declared `character_entity`, `entities`, `embedding`, and chronological `events`. It ends with a probe or legacy query. Event IDs are unique within a scenario; query IDs are unique throughout the fixture. Memory IDs cannot collide with declared entities or other admitted objects. Unknown fields reject. TOML duplicate keys reject, and non-finite TOML floats reject anywhere in the document with their key path before JSON conversion.

Declare entities with `external_id`, `label`, `entity_type` (`person`, `organization` or `location`) and `is_hub`. The character is the entity whose recall is measured. Every scene, including named declarations and inline scenes, must include that character as a participant, identified by a key or by a scoring-only `gold_entity` annotation.

Scenes live in the optional `scenes` map or inline on an event. Select one with `{kind = "named", name = "kitchen"}` or `{kind = "inline", scene = {...}}`. A scene has:

| Field | Shape and identity |
|---|---|
| `who` | Array of `{reference = {...}, gold_entity = "optional-declared-id"}`. Perceived references must be distinct. |
| `where` | Optional place reference. A `key` names a declared entity; a `setting` names a nonblank setting such as a kitchen, without inventing an entity. |
| `what` | Optional activity reference. A `key` must name an earlier authored thread or open loop when the scene is used. |
| `custom` | Optional string-to-string map; keys and values must be nonblank. |

Perceived references are `{by = "key", key = "id"}`, `{by = "name", text = "Jo"}`, or `{by = "description", text = "the visitor in the red coat"}`. `{by = "setting", key = "kitchen"}` is allowed only for `where`. Write-side scenes require keys or settings, never names or descriptions; probe scenes can use perceived names and descriptions. A participant's `gold_entity` must be declared and must agree with a key reference when both exist. It is removed from the input projection.

## Events and embeddings

Every event has a nonblank `event_id` and an RFC 3339 `timestamp`. Timestamps may be equal but cannot go backward. References to memories and activities must point to earlier admitted events.

| Event | Required fields beyond ID and timestamp | Optional fields |
|---|---|---|
| `experience` | `text`, `scene`; its event ID becomes the experience's memory ID | `speaker` (declared entity), `salience` (finite number in `[0,1]`) |
| `derive` | `memory` | `expected_warning` |
| `probe` | `query_id`, `scene` | `topic`, `assertions`, `measures` |

An experience creates an episode and an observation. An explicit salience value reaches both native drafts; absence preserves native defaults and is omitted from serialized input. Speaker is forwarded to the observation. Neither value is an unsupported feature.

An authored `memory` requires `subtype`, nonblank `text`, nonempty distinct `experiences` naming earlier experiences, and `about` containing distinct declared entities (possibly empty). Subtypes are `reflection`, `preference`, `relationship_note`, `open_loop`, `commitment`, `intention`, `character_signal` and `thread`. Optional `supersedes` names distinct earlier authored derived memories or threads. Optional `actor` and `counterpart` must appear together and name declared entities. Optional `due` is an RFC 3339 timestamp. Optional `trigger` is `{kind = "participant", entity = "jo"}` or `{kind = "topic", text = "Thursday dinner"}`. Supersession is forwarded as authored input; it is not an additional feature gate. `expected_warning` is scoring-only and accepts `near_verbatim_restatement` or `churning_chain`.

A probe may omit its topic entirely. A present topic must be nonblank; its embedding lookup uses trimmed text.

Lifecycle events use the same event identity and timestamp: `link` supplies `external_id`, `from_external_id`, `relation` and `to_external_id`; `forget` supplies `target_external_ids`, `suppress_derived_from_target` and `apply_to_derived_from_target`; `correct` supplies `target_external_id`, `replacement_external_id` and `replacement_text`; `restart` supplies `reopen_graph` and `reopen_stats`. At least one store must reopen and a later query or probe must follow a restart. For the `remember` and `query` shapes, see [fixture.rs](src/fixture.rs).

For deterministic authoring, use `provider = "controllable_similarity"` with `seed`, `vector_size`, `noise_magnitude`, `clusters` and `concepts`. Each cluster is a vector; each concept names its cluster and exact input strings. `own_concept = true` gives every otherwise-unassigned runtime input its own deterministic concept while preserving explicit groups. This supplies coverage, not semantic similarity: group texts explicitly when the narrative needs them to match. With `own_concept = false` (the default), the loader rejects uncovered inputs, including entity labels, normalized write texts, probe topics, textual scene references and topic triggers. Runtime normalization includes whitespace collapse and native derived-label prefix stripping. A frozen provider is `{provider = "frozen"}` and requires complete configured cache coverage before execution; see the [root README](../../README.md#generate-and-validate-frozen-real-embeddings) for store generation.

## Assertions and cue vocabulary

`assertions` defaults to empty arrays. Each check has an identity consisting of its event ID, assertion kind and the subject below. List reordering does not change identities; the order *inside* an `in_order` subject does. Referenced memories must be earlier authored experiences, derived memories or threads. Carried reasons organize recall measurements; use `cued` to require the specific retrieval cue as well.

| Assertion | Entry shape | Subject and meaning |
|---|---|---|
| `carried` | `{memory = "m", reason = "pair", section = "episodes"}`; section optional | Memory ID; admitted in a native pack, in the requested section if supplied. One carried entry per memory. |
| `in_order` | `["first", "second"]` | Full ordered sequence; at least two distinct carried memories in relative native pack order. Distinct sequences can share members. |
| `omitted` | `{memory = "m", reason = "resolution"}` | Memory ID; absent with the specified native omission reason. Cannot also be carried. Reasons: `resolution`, `supersession`, `suppression`. |
| `cued` | `{memory = "m", cue = "due"}` | `(memory, cue)`; that named cue occurred for that memory. |
| `not_cued` | `{memory = "m", cue = "date"}` | `(memory, cue)`; available native cue facts show that named cue did not occur. It does not assert omission. |
| `references` | `{participant = {by = "name", text = "Jo"}, resolution = {status = "resolved", entity = "jo"}}` | Full perceived participant reference, which must appear in this probe's `who`; compares native resolution. |
| `scenes` | `{memory = "m", scene = "kitchen"}` | `(memory, scene)`; a carried memory's native scene equals the named authored scene. Only one scene assertion per memory. |
| `elapsed_since_met` | `"jo"` | Counterpart entity ID; elapsed time from the most recent earlier authored experience containing both character and counterpart. Requires such a meeting. |
| `staleness` | `"m"` | Carried memory ID; age from its most recent supporting experience, not its derivation time. |

A derive's `expected_warning` adds a `write_warning` assertion whose subject is the warning kind. Duplicate subjects reject. The loader computes elapsed time and staleness; authors do not supply numerical answers. A scene assertion must match the experience's provenance. A derive inherits a single source scene only when all supporting experiences have the same perceived participants, place, activity and custom values; mixed-scene derivations cannot carry a scene assertion.

Reference resolution also accepts `{status = "ambiguous", candidates = ["jo-a", "jo-b"]}` (at least two distinct declared candidates, including any annotated gold entity) or `{status = "unknown"}`. A keyed participant can only resolve to its key. A resolved result must agree with any `gold_entity`; unknown means the native resolver did not establish identity, even if the author knows it.

Sections are `threads`, `episodes`, `observations`, `derived_memories`, `preferences`, `relationship_notes`, `open_loops`, `commitments` and `character_signals`. Checks and measurements inspect all native outcomes of a retrieval. Observations credit their parent experience; flattened retrieval items are not evidence of native section membership.

`reason` and `cue` share one closed vocabulary:

| Value | Intended recall route |
|---|---|
| `pair` | The character and another participant's shared history or relationship picture |
| `due` | A deadline |
| `date` | A dated memory or anniversary |
| `trigger` | An intention's participant or topic trigger |
| `activity` | The activity or thread being resumed |
| `own_day` | The character's own day |
| `recent_and_salient` | A recent, salient experience |
| `topic` | Topical content |

Current state is not a cue kind; it is the currency-filtered reading of the who and what cues, so use `pair` for a person and `activity` for a thread. The same memory may be checked for several cues, but duplicate `(memory, cue)` entries reject and the same tuple cannot be both `cued` and `not_cued`. Co-occurring cues do not prove each other: carry a due memory and assert `cued due` when a pair cue could also admit it. Unavailable cue facts fail both positive and negative checks if executed; they never become negative evidence.

## Measures and reports

Measures have no pass thresholds. Each probe reports:

| Measure | Calculation |
|---|---|
| `carried_recall_by_reason` | For each of the eight reasons, `expected` counts carried targets and `admitted` counts targets present in any native section. `recall = admitted / expected`; the optional carried section affects its assertion, not this recall count. |
| `bystander_context_share` | Distinct admitted authored experiences or derived memories listed in `measures.bystanders`, divided by all distinct admitted authored experiences and derived memories. Episode and observation count once under their experience; entities and threads are excluded. |
| `context_tokens` | Shared token counter applied to the rendered context. |

`bystanders` defaults to empty; entries must be distinct prior experiences or derived memories, cannot be carried, and cannot be entities or threads. Audit each label against every cue at the probe: no pair, due, date, trigger, activity, own-day, recent-and-salient or topic cue may call for that memory in the story. A different scene alone does not make it a bystander. A memory formed in the probe's place or setting is not a bystander either. Place has no cue kind in the vocabulary until a scenario needs to assert it. This is a narrative review rule; the loader cannot determine semantic cue eligibility. Negative expectations for a particular cue belong in `not_cued`; explicit ineligibility belongs in `omitted` with its reason. A recent unremarkable experience can remain unlabelled rather than being forced into either class.

Per-scenario `carried_recall_by_reason` and the aggregate map pool `(probe, carried memory)` counts among probes that ran, then divide; they do not average per-probe ratios. Thus 0/1 and 9/9 give 9/10, and a repeated target on another probe counts again. Skipped targets are excluded from pooled counts and remain visible in per-probe diagnostics. All eight reasons are present. Recall is null for an unexecuted probe/scenario or a reason with no carried targets; an executed miss is zero. Unexecuted admitted counts are null. Empty pooled counts have `expected = 0`. Bystander share is null when unexecuted or when no countable memories were admitted. Context tokens are null when unexecuted and zero for an executed empty context.

`report.json` includes every selected scenario, including those without trace rows. `outcome` contains status (`passed`, `failed`, `not_run`), missing features, assertion identities/results/reasons, per-probe measures and the omission-reason invariant. The invariant requires native lifecycle and currency omission counts to be accounted for by named reasons across every outcome; with no retrieval it is `not_run`. Passing requires executed checks and the invariant to hold; missing required native facts fail instead of being reconstructed from gold.

## Feature support and running

The loader derives needed features from actual fields and assertions; authors cannot declare them. The mapping is:

| Authored surface | Needed features |
|---|---|
| Experience scene | `write_scene`; present place/activity/custom adds `write_scene_where`, `write_scene_what`, `write_scene_custom` |
| Probe | `probe_scene`, `reference_time`; absent topic adds `no_topic` |
| Probe textual references | Role-specific `participant_name`, `participant_description`, `place_name`, `place_description`, `activity_name`, `activity_description` |
| Derive | `authored_derived_memory`; intention/preference add `intention_memory`/`preference_memory`; thread support/about/supersession adds `thread_provenance` |
| Direction, due, trigger | `direction`, `due_date`, `trigger` respectively |
| `cued` or `not_cued` | `cue_trace` |
| Reference, scene, elapsed, staleness assertions | `reference_trace`, `memory_scene_trace`, `elapsed_since_met`, `staleness` respectively |
| Omission, warning, section, order expectations | `omission_reasons`, `write_warnings`, `pack_sections`, `pack_order` respectively |

`SUPPORTED_SCENARIO_FEATURES` in [driver.rs](src/driver.rs) is the single statement of support. Any missing feature makes the entire scenario `not_run` before adapter construction, writes or retrieval; a supported prefix is not executed.

Run from the repository root into a fresh output directory:

```sh
cargo run -p cmem-eval-runner -- run continuity --dataset crates/cmem-eval-continuity/fixtures/situated_v1.toml --config configs/continuity_situated.toml --out .agent-work/situated/narrative/traces.jsonl
cargo run -p cmem-eval-runner -- run continuity --dataset crates/cmem-eval-continuity/fixtures/situated_loud_topic_v1.json --config configs/continuity_situated.toml --out .agent-work/situated/loud/traces.jsonl
cargo run -p cmem-eval-runner -- run continuity --dataset crates/cmem-eval-continuity/fixtures/situated_v1.toml --config configs/continuity_situated.toml --out .agent-work/situated/repeat/traces.jsonl
cargo run -p cmem-eval-runner -- compare-continuity .agent-work/situated/narrative/report.json .agent-work/situated/repeat/report.json
```

The run writes `header.json`, `traces.jsonl` and `report.json`; only executed scenarios get embedding bindings. An all-not-run command succeeds with an empty trace file. Stores are cleaned up by default. The comparison prints a JSON list of changed statuses, missing features, assertion identities/results/reasons, per-probe and pooled recall, and omission invariants. An empty list means those compared fields agree; it does not establish that a scenario ran. Use `diff` on trace files for retrieved identities and numeric metrics.

Regenerate the loud-topic fixture to a new path with `cargo run -p cmem-eval-continuity --bin generate_situated_loud_topic -- .agent-work/situated/generated.json`; it never overwrites an existing destination. Compare those bytes with the checked fixture. `cargo test -p cmem-eval-continuity` checks admission, generator reproducibility, mapping, assertions and reports. The [service-free smoke recipe](../../README.md#run-a-service-free-continuity-smoke) exercises an executed legacy scenario when the situated catalog is gated.
