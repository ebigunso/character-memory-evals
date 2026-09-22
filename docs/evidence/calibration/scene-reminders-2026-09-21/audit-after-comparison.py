import copy
import hashlib
import json
from pathlib import Path

root = Path(__file__).parent
read = lambda name: json.loads((root / name).read_text(encoding='utf-8'))
before = read('before-the-fix-native.json')
after = read('after-the-fix-native.json')
old = read('fourth-pin-before-p2.json')
middle = read('fourth-pin-after.json')
result = {'common_original_rows': 0, 'intermediate_numeric_changes': [], 'protected_files': []}
new_words = {'A stranger wearing a striped raincoat', 'An unfamiliar glass-roofed conservatory'}

def without_new_bindings(embedding, original):
    value = copy.deepcopy(embedding)
    for key in ['clusters', 'concepts']:
        assert set(value[key]) - set(original[key]) == new_words
        for word in new_words:
            value[key].pop(word)
    return value

for key in ['overlapping_input', 'reworded_input']:
    value = copy.deepcopy(before[key])
    value['scenario']['embedding'] = without_new_bindings(value['scenario']['embedding'], old[key]['scenario']['embedding'])
    value['probes'] = [probe for probe in value['probes'] if not probe['name'].startswith('unlived-scene-')]
    assert value == old[key], key
value = copy.deepcopy(before['keyless_input'])
value['embedding'] = without_new_bindings(value['embedding'], old['keyless_input']['embedding'])
assert value == old['keyless_input']
assert before['generated_input'] == old['generated_input']
assert before['paraphrase_geometry'] == old['paraphrase_geometry'] == middle['paraphrase_geometry']

def identity(row):
    return tuple(row[k] for k in ['probe', 'measured_kind', 'floor'])

def numbers(row):
    cohort = row['tracked_target_cohort']
    values = {'pack_slots': len(row['observed']['selected']),
              'topic_targets': cohort['pack']['survived_count'] if cohort else None}
    values.update({key: row.get('scene', {}).get(key) for key in
                   ['occasion_count', 'most_recent_n_occasions', 'same_day_occasions', 'other_day_occasions']})
    if row['probe'].startswith('unlived-'):
        values['exclusive_unlived_slots'] = sum(item['cue_kinds'] == [row['measured_kind']]
                                              for item in row['observed']['selected'])
    return values

for family in ['measurements', 'overlapping_measurements', 'reworded_measurements', 'keyless_measurements']:
    rows = {identity(row): row for row in before[family]['rows']}
    for row in old[family]['rows']:
        assert rows[identity(row)] == row, (family, identity(row))
        result['common_original_rows'] += 1
    for key in ['topic_only_control_cohort', 'topic_only_control_observed', 'topic_only_control_targets', 'reachability_controls']:
        if key in old[family]:
            assert old[family][key] == before[family][key], (family, key)
    final_rows = {identity(row): row for row in after[family]['rows']}
    for row in middle[family]['rows']:
        final = final_rows[identity(row)]
        if numbers(row) != numbers(final):
            result['intermediate_numeric_changes'].append({'family': family, 'probe': identity(row),
                                                         'intermediate': numbers(row), 'after': numbers(final)})
assert result['common_original_rows'] == 270

for protected in read('fourth-protected-hashes.json'):
    digest = hashlib.sha256((root.parents[2] / protected['path']).read_bytes()).hexdigest()
    assert digest == protected['expected'], protected['path']
    result['protected_files'].append({'path': protected['path'], 'sha256': digest, 'unchanged': True})

for pin, report in [('before', before), ('after', after)]:
    checks = 0
    for family in ['overlapping_measurements', 'reworded_measurements', 'opposed_identical_measurements', 'opposed_reworded_measurements']:
        for row in report[family]['rows']:
            if 'unlived_scene' not in row:
                continue
            cue = row['unlived_scene']
            selected = row['observed']['selected']
            control = cue['control_without_description']['selected']
            item_id = lambda item: (item['object']['object_type'], item['object']['id'])
            current_ids, control_ids = {item_id(item) for item in selected}, {item_id(item) for item in control}
            assert cue['cue_bearing_pack_slots'] == sum(row['measured_kind'] in item['cue_kinds'] for item in selected)
            assert cue['exclusive_cue_pack_slots'] == sum([row['measured_kind']] == item['cue_kinds'] for item in selected)
            assert {item_id(item) for item in cue['new_cue_slots_vs_description_removed']} == {
                item_id(item) for item in selected if row['measured_kind'] in item['cue_kinds'] and item_id(item) not in control_ids}
            displaced = {item_id(item) for key, items in cue['displaced_vs_description_removed'].items()
                         if key.startswith('section:') for item in items}
            assert displaced == control_ids - current_ids
            assert cue['native_scene_occasions'] == len(cue['occasions'])
            checks += 1
    assert checks == 80
    result[pin + '_unlived_counterfactual_checks'] = checks

result['original_inputs_preserved_except_added_probes_and_unused_bindings'] = True
(root / 'after-the-fix-audit.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
print(json.dumps(result, indent=2))
