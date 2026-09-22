const fs=require('fs'),assert=require('assert'),crypto=require('crypto');
const root='.agent-work/evals-worker/';
const json=p=>JSON.parse(fs.readFileSync(p));
const rows=p=>fs.readFileSync(p,'utf8').trim().split(/\r?\n/).filter(Boolean).map(JSON.parse);
const diffs=(a,b,p='')=>{if(JSON.stringify(a)===JSON.stringify(b))return [];if(a&&b&&typeof a==='object'&&typeof b==='object'&&Array.isArray(a)===Array.isArray(b)){return [...new Set([...Object.keys(a),...Object.keys(b)])].sort().flatMap(k=>diffs(a[k],b[k],p+'/'+k));}return [{path:p,before:a??null,after:b??null}];};
const census=r=>Object.fromEntries(Object.entries(r.scenarios).map(([id,v])=>[id,{status:v.outcome.status,missing_features:v.outcome.missing_features,assertions:v.outcome.assertions.reduce((a,x)=>(a[x.status]=(a[x.status]||0)+1,a),{})}]));
const comparisons={};
for(const kind of ['smoke','narrative']){
  comparisons[kind]={};
  for(const [label,a,b] of [['library-pin','prior-'+kind,'base-'+kind],['harness','base-'+kind,kind+'-a'],['repeat',kind+'-a',kind+'-b']]) {
    const left=rows(root+a+'/traces.jsonl'),right=rows(root+b+'/traces.jsonl');
    comparisons[kind][label]={headers:{before:json(root+a+'/header.json'),after:json(root+b+'/header.json')},trace_differences:diffs(left,right),report_differences:diffs(json(root+a+'/report.json'),json(root+b+'/report.json'))};
  }
}
const before=census(json(root+'base-narrative/report.json')),after=census(json(root+'narrative-a/report.json'));
const {execFileSync}=require('child_process');
const changed=execFileSync('git',['diff','--name-only','e15500eb5ae39ec1d18f255db3ca82247b7ccc66','8f49ddb','--','crates','Cargo.toml','Cargo.lock'],{encoding:'utf8'}).trim().split(/\r?\n/);
assert(changed.every(p=>['crates/cmem-eval-continuity/Cargo.toml','crates/cmem-eval-continuity/README.md','crates/cmem-eval-continuity/src/bin/calibrate_cue_floors.rs','crates/cmem-eval-continuity/src/bin/calibrate_cue_floors/descriptions.rs'].includes(p)));
const traceClock=/^\/\d+\/(latency_ms|retrieval_outcomes\/\d+\/pack\/(active_threads\/\d+|derived_memories\/\d+\/memory)\/(created_at|updated_at))$/;
const reportClock=/^\/aggregate\/latency\/latency_ms\/(mean|median|p50|p95)$/;
const gatePath=/^\/scenarios\/(d1-d8-morning-deadline|d11-c6-reunion-and-departure|d4-library-door|tasks-and-favors-after-a-year)\/outcome\/(missing_features\/\d+|assertions\/\d+\/reason|omission_reason_invariant\/reason)$/;
const attribution={source_control:{unchanged_executed_runtime:true,changed_files:changed,manifest_change:'Tokio moved from dev-dependencies to dependencies for the calibration binary; Cargo.lock and runner/adapter/continuity runtime sources are unchanged.'},comparisons:{}};
for(const [kind,entries] of Object.entries(comparisons)) for(const [label,x] of Object.entries(entries)) {
  x.header_differences=diffs(x.headers.before,x.headers.after);
  const counts={};
  for(const type of ['trace','report','header']) for(const d of x[type+'_differences']) {
    let category;
    if(type==='header') {
      if(d.path==='/library_commit'&&label==='library-pin') category='library_pin';
      else if(d.path==='/harness_commit'&&label!=='repeat') category='harness_provenance';
      else if(['/generated_at','/storage_root','/storage_root_sha256'].includes(d.path)) category='run_provenance';
    } else if((type==='trace'?traceClock:reportClock).test(d.path)) category='runtime_timing';
    else if(label==='library-pin'&&type==='trace') category='library_pin';
    else if(label==='harness'&&type==='report'&&kind==='narrative'&&gatePath.test(d.path)) category='harness_feature_gates';
    assert(category,`Unattributed ${kind}/${label}/${type}: ${d.path}`);
    d.attribution=category;
    counts[type]??={}; counts[type][category]=(counts[type][category]||0)+1;
  }
  attribution.comparisons[kind+'/'+label]=counts;
}
for(const id of Object.keys(before)) {
  assert.equal(before[id].status,after[id].status);
  assert.deepEqual(before[id].assertions,after[id].assertions);
  assert.deepEqual(after[id].missing_features,before[id].missing_features?.filter(x=>!['elapsed_since_met','resolution_omission'].includes(x)));
}
for(const pass of [1,2]) {
  const totals=json(root+`test-census-${pass}.json`).reduce((a,x)=>(a.passed+=x.passed,a.failed+=x.failed,a.filtered+=x.filtered,a),{passed:0,failed:0,filtered:0});
  assert.deepEqual(totals,{passed:389,failed:0,filtered:1});
}
for(const h of json(root+'protected-hashes.json')) assert.equal(crypto.createHash('sha256').update(fs.readFileSync('crates/cmem-eval-continuity/fixtures/'+h.path)).digest('hex'),h.expected);
for(const run of ['prior-smoke','prior-narrative','base-smoke','base-narrative','smoke-a','smoke-b','narrative-a','narrative-b']) assert(!fs.existsSync(root+run+'/stores'));
fs.writeFileSync(root+'attribution.json',JSON.stringify(attribution,null,2)+'\n');
fs.writeFileSync(root+'census-after.json',JSON.stringify(after,null,2)+'\n');
fs.writeFileSync(root+'evidence-differences.json',JSON.stringify(comparisons,null,2)+'\n');
console.log(JSON.stringify({census_before:before,census_after:after,difference_counts:Object.fromEntries(Object.entries(comparisons).map(([k,v])=>[k,Object.fromEntries(Object.entries(v).map(([type,x])=>[type,{trace:x.trace_differences.length,report:x.report_differences.length}]))]))},null,2));
