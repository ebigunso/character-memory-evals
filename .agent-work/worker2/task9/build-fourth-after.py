import hashlib
import json
import sys
from collections import Counter
from pathlib import Path

root = Path(__file__).parent
paths = dict(zip(['before', 'after'], map(Path, sys.argv[1:3]))) if len(sys.argv) > 1 else {
    'before': root / 'fourth-pin-before-p2.json', 'after': root / 'fourth-pin-after.json'}
output_stem = sys.argv[3] if len(sys.argv) > 3 else paths['after'].stem
reports = {name: json.loads(path.read_text(encoding='utf-8')) for name, path in paths.items()}
sources = {}
for name, path in paths.items():
    repeat = path.with_name(path.stem + '-repeat.json')
    complete = repeat.exists() and ('wrote ' in repeat.with_suffix('.log').read_text(encoding='utf-8'))
    identical = path.read_bytes() == repeat.read_bytes() if complete else None
    assert identical is True, f'Missing or different repeat: {repeat}'
    sources[name] = {'path': str(path), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(),
                     'header': reports[name]['header'], 'repeat_identical': identical}
before, after = reports.values()
middle_path = root / 'fourth-pin-after.json'
middle = json.loads(middle_path.read_text(encoding='utf-8')) if output_stem == 'after-the-fix' else None
if middle:
    sources['intermediate'] = {'path': str(middle_path), 'sha256': hashlib.sha256(middle_path.read_bytes()).hexdigest(),
        'header': middle['header'], 'repeat_identical': middle_path.read_bytes() == (root / 'fourth-pin-after-repeat.json').read_bytes()}
for key in before['header']:
    if key != 'library_commit':
        assert before['header'][key] == after['header'][key], f'Changed condition: {key}'
for key in ['generated_input', 'overlapping_input', 'reworded_input', 'keyless_input', 'paraphrase_geometry']:
    if key in before:
        assert before[key] == after[key], f'Changed generated input: {key}'

families = [('identical', 'overlapping_measurements'), ('reworded', 'reworded_measurements'), ('keyless', 'keyless_measurements')]
if 'opposed_input' in before:
    families += [('opposed-identical', 'opposed_identical_measurements'), ('opposed-reworded', 'opposed_reworded_measurements'), ('opposed-keyless', 'opposed_keyless_measurements')]
    assert before['opposed_input'] == after['opposed_input']
floors = [0, 1, 2, 3, 5]
def find(report, family, probe, kind, floor):
    return next(row for row in report[family]['rows'] if row['probe'] == probe and row['measured_kind'] == kind and row['floor'] == floor)

def intermediate(family, probe, kind, floor):
    if not middle or family not in middle:
        return None
    return next((row for row in middle[family]['rows'] if row['probe'] == probe and row['measured_kind'] == kind and row['floor'] == floor), None)

def summary(row):
    result = {'topic_targets_in_pack': row['tracked_target_cohort']['pack']['survived_count'],
              'target_cohort': row['tracked_target_cohort'],
              'pack_slots': len(row['observed']['selected'])}
    if 'scene' in row:
        scene = row['scene']
        assert scene['recorded_times_match_written_scene_times']
        result['scene'] = {key: scene[key] for key in ['occasion_count', 'most_recent_n_occasions',
            'same_day_occasions', 'other_day_occasions', 'available_same_day_occasions',
            'expected_most_recent_ids_for_returned_count', 'recorded_times_match_written_scene_times']}
        result['scene']['latest_n_and_available_same_day_basis'] = 'authored_scene_times'
        result['scene']['returned_times_basis'] = 'native_recorded_scene_times'
    if 'unlived_scene' in row:
        result['unlived_scene'] = {key:value for key,value in row['unlived_scene'].items() if key != 'control_without_description'}
    return result

def pollution(report, kind, floor):
    row = next(row for row in report['measurements']['rows'] if row['probe'] == f'unlived-{kind}' and row['floor'] == floor)
    selected = row['observed']['selected']
    weak = sum(slot['cue_kinds'] == [kind] for slot in selected)
    return {'exclusive_unlived_cue_slots': weak, 'all_pack_slots': len(selected),
            'share': weak / len(selected) if selected else None}

default_rows = []
sweep_rows = []
for label, family in families:
    for row in before[family]['rows']:
        identity = {key: row[key] for key in ['probe', 'measured_kind', 'floor']}
        other = find(after, family, *identity.values())
        if 'input' in row:
            assert row['input'] == other['input']
        comparison = {'family': label, **identity, 'before': summary(row), 'after': summary(other)}
        mid = intermediate(family, *identity.values())
        comparison['intermediate'] = summary(mid) if mid else None
        sweep_rows.append(comparison)
        if row['floor'] == 1 and row['measured_kind'] == 'topic':
            default_rows.append(comparison)
controls = {label: {name: report[family]['topic_only_control_cohort'] for name, report in reports.items()}
            for label, family in families}
if middle:
    for label, family in families:
        controls[label]['intermediate'] = middle[family]['topic_only_control_cohort'] if family in middle else None
pollution_rows = [{'kind': kind, 'floor': floor, **{name: pollution(report, kind, floor) for name, report in reports.items()}}
                  for kind in ['participant', 'place', 'topic'] for floor in floors]
if middle:
    for row in pollution_rows:
        row['intermediate'] = pollution(middle, row['kind'], row['floor'])
audit = {}
for name, report in reports.items():
    audit[name] = {}
    for family in ['measurements'] + [family for _, family in families]:
        group = report[family]
        observations = [row['observed'] for row in group['rows']] + [group['topic_only_control_observed']]
        observations += [row['unlived_scene']['control_without_description'] for row in group['rows'] if 'unlived_scene' in row]
        observations += [control[side] for control in group.get('reachability_controls', {}).values() for side in ['isolated', 'removed']]
        audit[name][family] = {'rows': len(group['rows']),
            'bounded_failures': sum(obs['telemetry']['graph_expansion']['bounded_failure_count'] for obs in observations),
            'root_count_mismatches': sum(len(obs['roots']) != obs['telemetry']['selected_graph_root_count'] for obs in observations if 'roots' in obs)}
        assert audit[name][family]['root_count_mismatches'] == 0
        assert audit[name][family]['bounded_failures'] == 0

before_pin = before['header']['library_commit'][:7]
after_pin = after['header']['library_commit'][:7]
conditions = f"Same harness {before['header']['harness_commit'][:7]} and seed 20260712; synthetic controllable 9D vectors; 48 shared occasions plus 8 topic-only targets graded 0.9–0.6; keyed writes minute-spaced, keyless 4/day over 12 days; candidate/root caps 48/12, episode/observation caps 8/16; default floors all 1, one floor swept at a time; library {before_pin} → {after_pin}; no paid calls."
definition = 'Pollution share = pack slots whose native cue set is exactly the deliberately unlived tested kind / all pack slots, measured in separate unchanged orthogonal probes; this is a synthetic weak-cue proxy, not natural-language relevance or causal floor admissions.'
result = {'sources': sources, 'conditions': conditions, 'pollution_definition': definition,
          'default_rows': default_rows, 'topic_alone_controls': controls, 'floor_sweeps': sweep_rows,
          'unlived_pollution': pollution_rows, 'native_audit': audit,
          'best_scene_surface_score_per_description': 'Not exposed as a best-score-per-description field in this pin; raw vector candidates include surface/score, which is not a pre-limit per-description best-score census.'}
result['paraphrase_geometry'] = before['paraphrase_geometry']
result['overlap_unlived'] = [row for row in sweep_rows if 'unlived_scene' in row['before']]
result['unlived_scene_limit'] = 'The unchanged orthogonal Remember corpus supplies no setting words or participant descriptions. After those cues move to scene-only search surfaces, its zero participant/place pollution has no stored description-surface competition; it does not establish a production weak-scene threshold.'
if output_stem == 'after-the-fix':
    result['integrity_audit'] = json.loads((root / 'after-the-fix-audit.json').read_text(encoding='utf-8'))
    result['id_order_audits'] = {name: json.loads(path.with_name(path.stem + '-audit.json').read_text(encoding='utf-8')) for name, path in paths.items()}
    result['retained_shared_episode_topic_scores'] = {}
    for label, family in families[:3]:
        values = {}
        for name, report in {**reports, 'intermediate': middle}.items():
            scores = [item['vector_score'] for item in report[family]['topic_only_control_observed']['candidates']
                      if item['object']['object_type'] == 'episode' and (item['external_id'] or '').startswith('shared-')]
            values[name] = [{'score': score, 'count': count} for score, count in sorted(Counter(scores).items())]
        result['retained_shared_episode_topic_scores'][label] = values
    result['validation'] = {'fmt': 'pass', 'clippy': 'pass', 'calibration_tests': '7 pass',
        'workspace': '388 unique tests pass; 3 fail (two isolated child test result lines also pass). Exact OS1314 case waived; two old-composite embedding driver tests are pin-induced and deferred by orchestrator at 2026-09-21T12:00:44Z.',
        'smoke': 'Two final-pin graded-similarity runs pass; current-format diff has zero identity/rank/metric/degradation changes. Old979 trace comparison cannot deserialize missing resolved_by; no cross-schema smoke equivalence claimed.',
        'review': 'Earlier Tier D approval covers 8f49ddb only; the added 7a8de19 and 97f6326 source commits have not received independent Tier D approval.'}
comparison_name = f'{output_stem}.json' if output_stem == 'after-the-fix' else f'{output_stem}-comparison.json'
(root / comparison_name).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

columns = ['before', 'intermediate', 'after'] if middle else ['before', 'after']
column_labels = [f'BEFORE {before_pin}', 'INTERMEDIATE b470b10', f'AFTER {after_pin}'] if middle else [f'BEFORE {before_pin}', f'AFTER {after_pin}']
def table(label):
    return ['| ' + label + ' | ' + ' | '.join(column_labels) + ' |', '|---|' + '---:|' * len(columns)]
def cell_targets(value):
    return f"{value['topic_targets_in_pack']}/8" if value is not None else 'not run'

lines = ['# Scene reminder calibration — before / after', '',
         'Default-floor topic targets improve **3/8 → 6/8** for identical and reworded scene descriptions in both native-ID orders (topic alone: 6/8). Keyless reminders return one latest occasion by authored chronology. Unknown descriptions still recall an occasion: the orthogonal zero is STRUCTURAL, not a similarity threshold.', '', conditions, ''] + table('Family / pressure')
for row in default_rows:
    if 'keyless' not in row['family']:
        pressure = row['probe'].removeprefix('overlap-').removesuffix('-sweep-topic')
        lines.append(f"| {row['family']} / {pressure} | " + ' | '.join(cell_targets(row.get(name)) for name in columns) + ' |')
for label, control in controls.items():
    lines.append(f"| {label} topic-alone | " + ' | '.join(f"{control[name]['pack']['survived_count']}/8" if control.get(name) else 'not run' for name in columns) + ' |')
lines += ['', 'Counts are the eight authored target episode objects; companion observations are separate slots. Original order means external labels ascend with time, while native UUID order is nonmonotonic. Opposed order makes native shared episode IDs strictly decrease with authored time; every nonidentity input is preserved. New probes/opposed variants were not run at b470b10; their intermediate entries are unavailable.', ''] + table('Keyless family / probe / measure')
def yes(value):
    return {True: 'yes', False: 'no', None: 'n/a'}[value]
for row in default_rows:
    if 'keyless' not in row['family']:
        continue
    for measure in ['occasion_count', 'most_recent_n_occasions', 'same_day_occasions', 'topic_targets_in_pack']:
        cells = []
        for name in columns:
            value = row.get(name)
            if value is None:
                cells.append('not run')
                continue
            scene = value['scene']
            cells.append(f"{value[measure]}/8" if measure == 'topic_targets_in_pack' else
                         yes(scene[measure]) if measure == 'most_recent_n_occasions' else
                         f"{scene[measure]}/{scene['available_same_day_occasions']}" if measure == 'same_day_occasions' else str(scene[measure]))
        label = {'occasion_count': 'native occasions', 'most_recent_n_occasions': 'latest N expected by AUTHORED times?',
                 'same_day_occasions': 'native same-day / AUTHORED available', 'topic_targets_in_pack': 'topic targets'}[measure]
        lines.append(f"| {row['family']} / {row['probe']} / {label} | " + ' | '.join(cells) + ' |')
lines += ['', 'Latest-N and same-day availability are expectations from AUTHORED scene times. Returned scenes/counts use native recorded values and every returned time matches its authored value. No public all-scenes census is captured; persisted times of unreturned occasions are not independently verified. Keyed and keyless chronology differ, so this is not an isolated causal estimate of removing keys.', '', definition, ''] + table('Default-floor orthogonal control')
def share(value):
    percentage = f"{value['share']:.1%}" if value['share'] is not None else 'n/a'
    return f"{value['exclusive_unlived_cue_slots']}/{value['all_pack_slots']} ({percentage})"
for row in pollution_rows:
    if row['floor'] == 1:
        label = row['kind'] + (' (STRUCTURAL)' if row['kind'] != 'topic' else '')
        lines.append(f"| {label} | " + ' | '.join(share(row[name]) for name in columns) + ' |')
lines += ['', result['unlived_scene_limit'], '', '## Unlived descriptions against the populated overlap stores', '',
          'These are the stranger/unfamiliar-place probes relevant to the future bound decision. Their query vectors have controlled weak scene similarity around 0.01; no production embedding similarity is inferred. Each cell shows native scene occasions; cue-bearing/exclusive pack slots; new cue slots and displaced pack slots versus the same query/floors with only this description removed; on-topic targets /8. Native cue overlap is not unique causal credit.', ''] + table('Family / default-floor probe')
def stranger_cell(value):
    if value is None:
        return 'not run'
    cue = value['unlived_scene']
    displaced = sum(len(items) for key, items in cue['displaced_vs_description_removed'].items() if key.startswith('section:'))
    return f"{cue['native_scene_occasions']} occasions; {cue['cue_bearing_pack_slots']}/{cue['exclusive_cue_pack_slots']} cue/exclusive slots; +{len(cue['new_cue_slots_vs_description_removed'])}/-{displaced} pack slots; {value['topic_targets_in_pack']}/8"
for row in result['overlap_unlived']:
    if row['floor'] == 1:
        lines.append(f"| {row['family']} / {row['probe']} | " + ' | '.join(stranger_cell(row.get(name)) for name in columns) + ' |')
lines += ['', 'The companion JSON retains every returned occasion, native scene, new slot and displaced identity with native scores for every floor. STRUCTURAL orthogonal zeros above must not be read as a stranger recalling nothing.', '',
          '## Floor sweeps', '', 'Each cell lists values in floor order **0, 1, 2, 3, 5**. All other floors remain 1. Scene reminders after the fix still admit at least one occasion when the floor is zero; zero does not disable a cue.', '',
          '| Family / pressure / swept floor | BEFORE topic targets /8 | AFTER topic targets /8 |', '|---|---|---|']
for label, family in families:
    probes = [(row['probe'], row['measured_kind']) for row in before[family]['rows'] if row['floor'] == 0]
    for probe, kind in probes:
        cells = [', '.join(str(summary(find(report, family, probe, kind, floor))['topic_targets_in_pack']) for floor in floors)
                 for report in reports.values()]
        lines.append(f"| {label} / {probe} / {kind} | " + ' | '.join(cells) + ' |')
lines += ['', '| Keyless family / probe / swept floor | BEFORE native occasions | AFTER native occasions | BEFORE latest-N by AUTHORED times | AFTER latest-N by AUTHORED times |', '|---|---|---|---|---|']
for label, family in families:
    if 'keyless' not in label:
        continue
    for probe, kind in [(row['probe'], row['measured_kind']) for row in before[family]['rows'] if row['floor'] == 0]:
        values = {name: [find(report, family, probe, kind, floor)['scene'] for floor in floors] for name, report in reports.items()}
        cells = [', '.join(str(scene['occasion_count']) for scene in values[name]) for name in reports]
        cells += [', '.join(yes(scene['most_recent_n_occasions']) for scene in values[name]) for name in reports]
        lines.append(f"| {label} / {probe} / {kind} | " + ' | '.join(cells) + ' |')
lines += ['', '| Populated-store unlived probe | BEFORE occasions / cue slots / displaced pack slots | AFTER occasions / cue slots / displaced pack slots |', '|---|---|---|']
for label, family in families:
    for row in before[family]['rows']:
        if row['floor'] != 0 or 'unlived_scene' not in row:
            continue
        cells = []
        for report in reports.values():
            cues = [find(report, family, row['probe'], row['measured_kind'], floor)['unlived_scene'] for floor in floors]
            counts = [','.join(str(cue[key]) for cue in cues) for key in ['native_scene_occasions', 'cue_bearing_pack_slots']]
            counts.append(','.join(str(sum(len(items) for key, items in cue['displaced_vs_description_removed'].items() if key.startswith('section:'))) for cue in cues))
            cells.append(' / '.join(counts))
        lines.append(f"| {label} / {row['probe']} | " + ' | '.join(cells) + ' |')
lines += ['', '| Unlived cue / swept floor | BEFORE pollution, floors 0/1/2/3/5 | AFTER pollution, floors 0/1/2/3/5 |', '|---|---|---|']
for kind in ['participant', 'place', 'topic']:
    cells = ['; '.join(share(pollution(report, kind, floor)) for floor in floors) for report in reports.values()]
    lines.append(f"| {kind} | " + ' | '.join(cells) + ' |')
lines += ['', 'All intermediate candidate/root/pack cohorts, keyless same-day counts for every sweep, native traces and exact identities remain in the raw reports and comparison JSON. Controlled paraphrase geometry is unchanged and cannot select a production similarity bound.', '',
          result['best_scene_surface_score_per_description'], '',
          'Repeat identity: ' + '; '.join(f"{name}: {'yes' if source['repeat_identical'] else 'pending'}" for name, source in sources.items()) + '.', '',
          f'Inputs/configuration/source hashes match across the final BEFORE/AFTER pins; only the library commit differs in their headers. The b470b10 intermediate uses the earlier approved harness for common original-order probes; no new-case intermediate result is inferred. All created run stores are cleaned by the runner. Native bounded failures and root-count audits are retained in {comparison_name}.', '',
          f"Before SHA-256: `{sources['before']['sha256']}`. After SHA-256: `{sources['after']['sha256']}`."]
lines += ['', '## Controlled paraphrase distributions (unchanged across pins)', '',
          '| Kind | Same-referent cosine distribution | Different-referent cosine distribution | Separable band? |', '|---|---|---|---|']
for distribution in before['paraphrase_geometry']['distributions']:
    same, different = distribution['same_referent_cosines'], distribution['different_referent_cosines']
    lines.append(f"| {distribution['kind']} | n={len(same)}, {min(same):.4f}–{max(same):.4f} | n={len(different)}, {min(different):.4f}–{max(different):.4f} | {yes(distribution['strictly_separable_band'])} |")
lines += ['', before['paraphrase_geometry']['method'], '',
          '| Kind | Threshold | False-remind rate BEFORE = AFTER | Missed-remind rate BEFORE = AFTER |', '|---|---:|---:|---:|']
for distribution in before['paraphrase_geometry']['distributions']:
    for threshold in distribution['threshold_sweep']:
        if threshold['threshold'] in [0.5, 0.7, 0.8, 0.9, 0.95, 1.0]:
            lines.append(f"| {distribution['kind']} | {threshold['threshold']:.2f} | {threshold['false_remind_rate']:.1%} | {threshold['missed_remind_rate']:.1%} |")
if output_stem == 'after-the-fix':
    lines += ['', '## ID/time validity and intermediate pin', '',
              'External labels originally ascend with authored time, but native UUID order is nonmonotonic. The new variants prepare all 48 native IDs through the public adapter without committing planning writes, then make native ID order strictly oppose time. The audit verifies every nonidentity input is preserved and checks prepared IDs against the observed native traces.', '',
              'TOPIC scores are not one 48-way tie. The original merged scene/body embeddings had graded topic similarity. After scene separation, concept-seeded discrete noise creates several exactly tied subsets near zero; episode/observation companions can also tie. The comparison JSON records score-frequency distributions for retained shared episode candidates, not a complete pre-limit census. The unchanged result under opposed native IDs rules out ascending ID/time order as the explanation for these families.', '',
              'All 270 original BEFORE rows and their controls exactly match the approved 8f49ddb capture. Their original writes/probes and embedding assignments are unchanged; two new unused bindings and four extra probes were added. Across those common rows, b470b10 and 63f176f have identical target counts, pack counts, occasion/latest/day counts and orthogonal exclusive-cue counts. Raw traces and scores remain available; no conclusion about unmeasured cases is implied.', '',
              '## Validation and remaining migration work', '',
              'Both final pins ran all 515 rows twice with byte-identical outputs. Native bounded-failure counts and root-count mismatches are zero, including description-removed controls. Eighty unlived counterfactuals per pin and six protected fixture hashes pass the audit. Formatting, Clippy and seven calibration checks pass.', '',
              'The final-pin workspace suite has 388 unique passes and 3 failures: the previously waived Windows OS1314 symlink test, plus `driver::tests::scene_slice_preserves_authored_input_and_checks_native_results` and `driver::tests::situated_writes_reject_degraded_native_outcomes`. These two tests assume the old composite embedding: the first lacks a separate assignment for `Quiet observatory`; the second removes a composite string no longer embedded and therefore no longer triggers degradation. The orchestrator deferred that scenario-driver migration at 2026-09-21T12:00:44Z and directed measurement/report completion without editing them. The workspace suite is not reported green.', '',
              'The README service-free smoke ran twice at 63f176f and its current-format diff has zero identity/rank/metric/degradation changes. The old 979 trace cannot be read by the new diff command because `resolved_by` is missing; no cross-schema smoke equivalence is claimed. Earlier Tier D approval is for 8f49ddb; the new two source commits are not independently approved.', '',
              'Evidence: `after-the-fix-audit.json`, both `*-native-audit.json` files, `final-fmt.log`, `final-clippy.log`, `final-workspace.log`, and `after-the-fix-smoke-repeat-diff.log`. No protected fixture, library source, rule or ADR was changed. No push was performed.']
(root / f'{output_stem}.md').write_text('\n'.join(lines) + '\n', encoding='utf-8')
print(json.dumps({'defaults':default_rows,'pollution':[row for row in pollution_rows if row['floor']==1],
                  'topic_alone':{label:{name:cohort['pack']['survived_count'] if cohort else None for name,cohort in control.items()} for label,control in controls.items()},
                  'after_repeat_identical':sources['after']['repeat_identical']},ensure_ascii=False))
