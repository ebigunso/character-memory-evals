import copy
import hashlib
import json
import sys
from pathlib import Path

path = Path(sys.argv[1])
report = json.loads(path.read_text(encoding='utf-8'))
repeat = path.with_name(path.stem + '-repeat.json')
assert path.read_bytes() == repeat.read_bytes()
assert not path.with_suffix('.stores').exists() and not repeat.with_suffix('.stores').exists()
audit = {'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'header': report['header'], 'families': {}}
for variant in report['opposed_input']:
    label = variant['family']
    order = variant['id_order']
    assert len(order) == 48
    assert all(a['prepared_native_episode_id'] > b['prepared_native_episode_id']
               and a['authored_scene_time'] < b['authored_scene_time'] for a, b in zip(order, order[1:]))
    ids = {row['external_id']: row['prepared_native_episode_id'] for row in order}
    assert len(ids) == 48
    if label == 'keyless':
        normalized = copy.deepcopy(variant['input'])
        original = report['keyless_input']
        for changed, source in zip(normalized['writes'], original['writes'], strict=True):
            for key in ['episode_external_id', 'observation_external_id']:
                changed[key] = source[key]
    else:
        normalized = copy.deepcopy(variant['scenario'])
        original = report['overlapping_input' if label == 'identical' else 'reworded_input']['scenario']
        for changed, source in zip(normalized['events'], original['events'], strict=True):
            changed['event_id'] = source['event_id']
    assert normalized == original, f'{label}: non-identity input changed'
    family = report[f'opposed_{label}_measurements']
    observations = [row['observed'] for row in family['rows']] + [family['topic_only_control_observed']]
    observed_ids = set()
    for observation in observations:
        assert len(observation['roots']) == observation['telemetry']['selected_graph_root_count']
        assert observation['telemetry']['graph_expansion']['bounded_failure_count'] == 0
        for stage in ['candidates', 'roots', 'selected']:
            for item in observation[stage]:
                if item['object']['object_type'] == 'episode' and item['external_id'] in ids:
                    assert item['object']['id'] == ids[item['external_id']]
                    observed_ids.add(item['external_id'])
    audit['families'][label] = {'strictly_opposed_prepared_ids': True, 'all_nonidentity_inputs_equal': True,
        'native_episode_ids_verified_in_observed_traces': len(observed_ids), 'rows': len(family['rows']),
        'bounded_failures': 0, 'root_count_mismatches': 0}
path.with_name(path.stem + '-audit.json').write_text(json.dumps(audit, indent=2) + '\n', encoding='utf-8')
print(json.dumps({k:v for k,v in audit.items() if k != 'header'}, indent=2))
