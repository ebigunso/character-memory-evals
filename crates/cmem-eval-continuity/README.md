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
  {external_id = "mara", label = "Mara", is_hub = false},
  {external_id = "jo", label = "Jo", is_hub = false},
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

Declare entities with `external_id`, `label` and `is_hub`. The label becomes the exact text of an application-given naming belief about the native notion, with a `known_as` assertion. The character is the entity whose recall is measured. Every scene, including named declarations and inline scenes, must include that character as a participant, identified by a key or by a scoring-only `gold_entity` annotation.

Scenes live in the optional `scenes` map or inline on an event. Select one with `{kind = "named", name = "kitchen"}` or `{kind = "inline", scene = {...}}`. A scene has:

| Field | Shape and identity |
|---|---|
| `who` | Array of `{reference = {...}, gold_entity = "optional-declared-id"}`. Perceived references must be distinct. |
| `where` | Optional place reference. A `key` names a declared entity; a `setting` names a nonblank setting such as a kitchen, without inventing an entity. |
| `what` | Optional activity reference. A `key` must name an earlier authored thread or open loop when the scene is used. |
| `custom` | Optional string-to-string map; keys and values must be nonblank. |

Perceived references are `{by = "key", key = "id"}`, `{by = "name", text = "Jo"}`, or `{by = "description", text = "the visitor in the red coat"}`. `{by = "setting", key = "kitchen"}` is allowed only for `where`. Both experience and probe scenes accept perceived names and descriptions. The scene slice forwards place keys as setting keys and place words as setting words; activity remains unsupported. A participant's `gold_entity` must be declared and must agree with a key reference when both exist. It is removed from the input projection.

## Events and embeddings

Every event has a nonblank `event_id` and an RFC 3339 `timestamp`. Timestamps may be equal but cannot go backward. References to memories and activities must point to earlier admitted events.

| Event | Required fields beyond ID and timestamp | Optional fields |
|---|---|---|
| `experience` | `text`, `scene`; its event ID becomes the experience's memory ID | `speaker` (declared entity), `salience` (finite number in `[0,1]`) |
| `derive` | `memory` | `expected_warning` |
| `probe` | `query_id`, `scene` | `topic`, `assertions`, `measures` |

An experience creates an episode and an observation. An explicit salience value reaches both native drafts; absence preserves native defaults and is omitted from serialized input. Speaker is forwarded to the observation. Neither value is an unsupported feature.

An authored `memory` requires `subtype`, nonblank `text`, nonempty distinct `experiences` naming earlier experiences, and `about` containing distinct declared entities (possibly empty). Subtypes are `reflection`, `preference`, `relationship_note`, `open_loop`, `commitment`, `intention`, `character_signal` and `thread`. Optional `supersedes` names distinct earlier authored derived memories; thread targets are rejected at load. Optional `actor` and `counterpart` must appear together and name declared entities. Optional `due` is an RFC 3339 timestamp. Optional `trigger` is `{kind = "participant", entity = "jo"}` or `{kind = "topic", text = "Thursday dinner"}`. Supersession is forwarded as authored input; it is not an additional feature gate. `expected_warning` is scoring-only and accepts `near_verbatim_restatement` or `churning_chain`.

A probe may omit its topic entirely. A present topic must be nonblank; its embedding lookup uses trimmed text. Every probe needs at least one assertion or a nonempty `measures.bystanders` list; an empty probe is rejected at load.

Lifecycle events use the same event identity and timestamp: `link` supplies `external_id`, `from_external_id`, `relation` and `to_external_id`; `forget` supplies `target_external_ids`, `suppress_derived_from_target` and `apply_to_derived_from_target`; `correct` supplies `target_external_id`, `replacement_external_id` and `replacement_text`; `restart` supplies `reopen_graph` and `reopen_stats`. Both graph and statistics stores must reopen and a later `query` must follow a restart; probe-only re-measurement is rejected at load. For the `remember` and `query` shapes, see [fixture.rs](src/fixture.rs).

For deterministic authoring, use `provider = "controllable_similarity"` with `seed`, `vector_size`, `noise_magnitude`, `clusters` and `concepts`. Each cluster is a vector; each concept names its cluster and exact input strings. `own_concept = true` gives every otherwise-unassigned runtime input its own deterministic concept while preserving explicit groups. This supplies coverage, not semantic similarity: group texts explicitly when the narrative needs them to match. With `own_concept = false` (the default), the loader rejects uncovered inputs, including entity labels, normalized write texts, probe topics, textual scene references and topic triggers. Write inventory follows native whitespace collapse: episode scene words append a `Setting:` line and then a `With:` line per participant; keys and custom values add no embedding text. Probe text trims outer whitespace only. The provider strips native surface labels for lookup, and an unexpected native input fails instead of silently assigning a vector. A frozen provider is `{provider = "frozen"}` and requires complete configured cache coverage before execution; see the [root README](../../README.md#generate-and-validate-frozen-real-embeddings) for store generation.

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
| `place` | The perceived place or setting |
| `due` | A deadline |
| `date` | A dated memory or anniversary |
| `trigger` | An intention's participant or topic trigger |
| `activity` | The activity or thread being resumed |
| `own_day` | The character's own day |
| `recent_and_salient` | A recent, salient experience |
| `topic` | Topical content |

Current state is not a cue kind; it is the currency-filtered reading of the who and what cues, so use `pair` for a person and `activity` for a thread. The same memory may be checked for several cues, but duplicate `(memory, cue)` entries reject and the same tuple cannot be both `cued` and `not_cued`. Co-occurring cues do not prove each other: carry a due memory and assert `cued due` when a pair cue could also admit it. At library `9ff86d6`, `pair` maps to native `participant`; `topic`, `place` and `activity` map directly. Cue facts are the union of selected section assignments across native outcomes, including an experience's observation. Omitted assignments do not supply a cue fact. Unavailable cue facts fail both positive and negative checks if executed; they never become negative evidence.

## Measures and reports

Measures have no pass thresholds. Each probe reports:

| Measure | Calculation |
|---|---|
| `carried_recall_by_reason` | For each cue-vocabulary reason, `expected` counts carried targets and `admitted` counts targets present in any native section. `recall = admitted / expected`; the optional carried section affects its assertion, not this recall count. |
| `bystander_context_share` | Distinct admitted authored experiences or derived memories listed in `measures.bystanders`, divided by all distinct admitted authored experiences and derived memories. Episode and observation count once under their experience; entities and threads are excluded. |
| `context_tokens` | Shared token counter applied to the rendered context. |

`bystanders` defaults to empty; entries must be distinct prior experiences or derived memories, cannot be carried, and cannot be entities or threads. Audit each label against every cue at the probe: no pair, place, due, date, trigger, activity, own-day, recent-and-salient or topic cue may call for that memory in the story. A different scene alone does not make it a bystander. A memory formed in the probe's place or setting is not a bystander either. This is a narrative review rule; the loader cannot determine semantic cue eligibility. Negative expectations for a particular cue belong in `not_cued`; explicit ineligibility belongs in `omitted` with its reason. A recent unremarkable experience can remain unlabelled rather than being forced into either class.

Per-scenario `carried_recall_by_reason` and the aggregate map pool `(probe, carried memory)` counts among probes that ran, then divide; they do not average per-probe ratios. Thus 0/1 and 9/9 give 9/10, and a repeated target on another probe counts again. Skipped targets are excluded from pooled counts and remain visible in per-probe diagnostics. All nine reasons are present. Recall is null for an unexecuted probe/scenario or a reason with no carried targets; an executed miss is zero. Unexecuted admitted counts are null. Empty pooled counts have `expected = 0`. Bystander share is null when unexecuted or when no countable memories were admitted. Context tokens are null when unexecuted and zero for an executed empty context.

`report.json` includes every selected scenario, including those without trace rows. `outcome` contains status (`passed`, `failed`, `not_run`), missing features, assertion identities/results/reasons, per-probe measures and the omission-reason invariant. The invariant requires native lifecycle and currency omission counts to be accounted for by named reasons across every outcome; with no retrieval it is `not_run`. Passing requires executed checks and the invariant to hold; missing required native facts fail instead of being reconstructed from gold.

## Feature support and running

The loader derives needed features from actual fields and assertions; authors cannot declare them. The mapping is:

| Authored surface | Needed features |
|---|---|
| Experience scene | `write_scene`; present place/activity/custom adds `write_scene_where`, `write_scene_what`, `write_scene_custom` |
| Probe | `probe_scene`, `reference_time`, `omission_reasons`; absent topic adds `no_topic`; an activity key adds `probe_activity` |
| Probe textual references | Role-specific `participant_name`, `participant_description`, `place_name`, `place_description`, `activity_name`, `activity_description` |
| Derive | `authored_derived_memory`; intention/preference add `intention_memory`/`preference_memory`; thread support/about/supersession adds `thread_provenance` |
| Direction, due, trigger | `direction`, `due_date`, `trigger` respectively |
| `cued` or `not_cued` | `cue_trace`; due/date/trigger/own-day/recent-and-salient add `due_cue`/`date_cue`/`trigger_cue`/`own_day_cue`/`recent_and_salient_cue`; `not_cued pair` adds `pair_counterpart_cue` until the library distinguishes the self |
| Reference, scene, elapsed, staleness assertions | `reference_trace`, `memory_scene_trace`, `elapsed_since_met`, `staleness` respectively |
| Resolution assertion on a participant description | `description_reference_resolution`: the library reports only a content cue for descriptions, not resolved, ambiguous or unknown |
| Omission by resolution | `resolution_omission` (unsupported until the library reports end of currency) |
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

## Cue floor calibration

After pinning the sibling CharacterMemory checkout, run the generated calibration with one new report path. Create the output parent directory first. Use `cargo run` after each pin change so the linked library is rebuilt; the report records that checkout's commit. An existing report or store directory is refused.

```sh
cargo run --offline --release -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/worker/floors-a.json
cargo run --offline --release -p cmem-eval-continuity --bin calibrate_cue_floors -- .agent-work/worker/floors-b.json
git diff --no-index -- .agent-work/worker/floors-a.json .agent-work/worker/floors-b.json
```

This experiment is separate from the situated scenarios and has no behavioral pass/fail assertions. It generates a seeded corpus, ingests it with the continuity driver, and retrieves through the existing adapter with the core floor override. It sweeps participant, place, activity and topic floors through 0, 1, 2, 3 and 5; the other floors, graph depth and caps retain native defaults. Cases include a loud topic, each competing kind, all kinds together, unrelated scene words and a thread with 16 members. Isolated-cue and removed-cue controls establish whether each target can measure starvation; otherwise the starvation value is null. Metadata targets never enter retrieval inputs.

A separate generated overlapping-pressure corpus writes 48 experiences with setting words and participant descriptions through the actual scene driver. Their indexed episode text includes native `Setting` and `With` lines; controlled vectors make those episodes strong scene matches and weak topic matches. Eight further experiences are strong topic-only matches, graded from cosine 0.9 to 0.6. Place, participant and combined scene probes sweep all four floors (the absent activity is a control). Each row counts how many of the eight authored episodes survive candidate merge, root selection and the pack independently, and lists the missing identities and other scored occupants. Companion observations remain separate. The topic-only control has the same census, so native capacity losses without scene competition are visible. This family has its own input hash and measurements in the same report, so a library pin comparison can expose overlap effects that the orthogonal corpus does not cover.

The reworded family keeps those controls and chooses among three authored descriptions per referent by seed, reserving a fourth wording for the probe. Its query has a distinct authored base from every stored wording, with query-to-description cosines about 0.90, 0.957 and 0.973. The identical family is the exact-match control. `reworded_geometry.pairs` records actual provider-emitted vectors and their cosine for each query against every stored description and normalized episode surface. Normalized episode bases still depend on episode index, independently of wording, holding topic pressure fixed. This band is authored experimental input, not a production embedder measurement. Earlier versions gave the held-out wording the same base as one stored description; agreement with the identical family and same-day-by-descriptions readings from those versions do not establish robustness to freely worded perception. The keyless family uses the existing public adapter's prepare, validate and commit operations with time and descriptions only: no entity registration, participant keys, setting keys or speaker keys. Memory external IDs and namespaces are storage bookkeeping, not perceived identities. It writes 48 occasions over 12 days and eight topic targets, then probes topic plus scene, scene alone, and the last day by descriptions. Latest-N membership and the available same-day denominator are expectations from authored scene times. Returned same-day counts use the selected episodes' native recorded scenes, and each returned occasion's native time is checked against its authored value. The public adapter provides no all-scenes census here, so persisted times for unreturned occasions are not independently verified. Results also include the topic-only denominator. These are measurements without behavioral assertions; they do not claim consolidation or a temporal filter. The scenario language still requires a character entity, so this family deliberately stays outside that language under the task owner's ruling.

The paraphrase diagnostic measures vectors emitted by the existing controllable-similarity provider for authored same-referent and different-referent description pairs. It records both distributions and a threshold sweep of false-remind and missed-remind rates. This geometry is controlled input, including deliberately overlapping cases, not empirical evidence about a production model; the committed frozen stores do not contain these new descriptions, and the command makes no paid calls or embedding-store changes. A production similarity bound requires separately captured real-model paraphrase vectors. Raw traces retain all native fields; a missing best scene-surface score per description is reported as unavailable.

Each overlapping family also runs with shared episode IDs opposed to authored time. The adapter maps external IDs to native UUIDs, so reversing external labels alone would not control native order. The runner obtains the existing IDs from uncommitted public prepared plans, sorts them descending, and permutes only the occasion identities while keeping namespace, content, scene times, descriptions, vectors and probes fixed; companion observation IDs follow their occasion. Those planning namespaces are cleaned before ingestion. The original externally ascending order and the opposed native episode order remain separate results with their input hashes and explicit prepared-ID/authored-time mapping. Neither order implies that topic scores tie: the old scene-bearing vectors have graded topic components, while body-only vectors share a background base with small seeded concept noise.

The overlap stores also receive stranger and unfamiliar-place description probes, with and without a topic, in both wording families and both ID orders. Their authored query vectors weakly match the stored scene descriptions at cosine about 0.01; this is controlled pressure, not a production similarity estimate. Each floor reports native cue-bearing and exclusive slots, the returned occasions and recorded scenes, new slots and displaced identities/scores against the same query with only that description removed. Shared cue membership is not unique causal credit. The older orthogonal Remember corpus has no description surfaces, so its zero scene-cue pollution after surface separation is a structural control, not evidence that a stranger recalls nothing.

The single JSON report contains the generated input, configuration and hashes, both commits, executed/not-run census, native traces, pack membership, stage/section displacement sets against the same probe at floor zero, and available native scores. Multiple floor admissions cannot be uniquely paired to displaced objects from this trace; admissions refer to their shared displacement group. Root ordering scores and some direct-versus-inherited cue origins are unavailable and are not reconstructed from fixture labels. Topic root counts mean membership in the independent topic-only candidate control, while topic pack counts read native cue sets and can overlap other kinds. Native telemetry records incomplete or bounded retrieval. Saturated explicit place roots remain a later library state-slice experiment.

The time and prospective-memory extension adds dated experiences, eight graded topic targets, an older salient occasion and a latest occasion with a distinct, unlinked observation. It records time-root attempts from native trace, authored latest-N expectations, selected native scene times, topic survival and pack membership/rank/score deltas against the same store's topic-alone control. The existing keyless same-day-by-descriptions measurements remain in `keyless_measurements` and `opposed_keyless_measurements` for continuity. Topic-alone includes the fixed scene time, so pollution from adding recency is assessed by comparing the same control across library pins, not by pretending that control disables a time route.

The prospective store includes obligations in both directions, two with native resolution links, due instants on either side of the reference time and own promises without counterparts, under pressure from 24 newer, more salient beliefs and eight topic experiences. At the before pin `63f176f`, recency floors, ranges, date-match, actor/counterpart assertions, character identity, due instants and trigger/due routes are unavailable. Their full cases are explicitly `not_run`; a separately labelled parent control executes only the supported scene/topic and directionless writes. Typed planned inputs retain the intended range endpoints, roles, identity and due instants for later wiring at the public adapter boundary. Unsupported keys are never silently sent to serde. Native resolution is reported only for included memories; absent direction/due fields remain null, and the authored local-day due classification is explicitly hypothetical. The actual write inputs and planned additions are recorded separately.

Both new families run in their original identity order and with native IDs descending as authored times increase; derived-memory IDs also oppose salience. Native IDs come from the original public ingestion and are checked against the second ingestion, without duplicating the adapter's identity recipe. Only storage identities and their references change; the content, times, vectors and queries stay fixed. The extension has its own complete input hash under `time_prospective_input_sha256`.

The time store also contains an open loop with three source episodes whose authored creation times oppose their scene times. Its activity probe sets the graph-root cap to two and the episode section cap to one, reporting retained source roots and returned native scene/creation times. To run the new slices plus the existing keyless same-day controls, append `--slices-only` after the output path; excluded measurement groups are null and the header records the mode. The default runs every family.

When the pinned native trace supplies `scene_cue_searches`, `native_paraphrase_scores` records the same-referent reworded and different-referent best-score distributions from the reworded overlap family's single-kind probes at floor one. It keeps the search's native references and best score, which precedes occasion selection and graph eligibility. A joined participant search contributes one sample, not one per reference. Missing trace fields are unavailable; null scores mean no fetched match. The original and opposed ID results are reported separately and are repeats of the same geometry, not additional independent samples. These native readings are an overlap-corpus sample of two searches per class and ID order; they are not paired with the separate 56-pair authored vector diagnostic. Both describe synthetic geometry.

The vectors create controlled score pressure, including a deliberately weak nearest neighbour for unrelated words; they do not estimate natural-language relevance thresholds. Generated inputs use fixed times, and reports exclude native write-construction timestamps and runtime timing. Every created store is removed on completion or a runtime error. Compare the whole reports for a deterministic repeat; these are working measurements for the library owner's floor decision, not sealed benchmark claims.
