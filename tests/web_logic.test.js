const assert = require('node:assert/strict');
const { isAllowedForTask, gateStatus, selectBestEligible, csvCell, groupByClass } = require('../web/logic.js');

const gate = { deterministic_pass_required:true, minimum_human_score:4, maximum_latency_ms:1000 };

assert.equal(gateStatus({execution_status:'completed',deterministic_pass:true,latency_ms:500,human_quality_score:null,quality_gate:gate}), 'pending_human_review');
assert.equal(gateStatus({execution_status:'completed',deterministic_pass:true,latency_ms:1001,human_quality_score:5,quality_gate:gate}), 'rejected');
assert.equal(gateStatus({execution_mode:'live',execution_status:'completed',deterministic_pass:true,latency_ms:500,human_quality_score:4,quality_gate:gate}), 'eligible');
assert.equal(gateStatus({execution_mode:'demo',execution_status:'completed',deterministic_pass:true,latency_ms:500,human_quality_score:5,quality_gate:gate}), 'demo_only');
assert.equal(selectBestEligible([
  {model_id:'fast-unreviewed',execution_status:'completed',deterministic_pass:true,latency_ms:100,human_quality_score:null,quality_gate:gate,estimated_cost_per_1000_tasks:0},
  {model_id:'approved',execution_mode:'live',execution_status:'completed',deterministic_pass:true,latency_ms:500,human_quality_score:5,quality_gate:gate,estimated_cost_per_1000_tasks:2}
]).model_id, 'approved');
assert.equal(isAllowedForTask({allowed_data:['public','internal']},{data_classification:'confidential'}), false);
assert.equal(csvCell('=HYPERLINK("bad")').startsWith('"\'='), true);

// A challenger that regresses against the champion is never the best eligible route.
assert.equal(selectBestEligible([
  {model_id:'cheap-regressed',execution_mode:'live',execution_status:'completed',deterministic_pass:true,latency_ms:100,human_quality_score:5,quality_gate:gate,estimated_cost_per_1000_tasks:0,regressed_vs_champion:true},
  {model_id:'champion',execution_mode:'live',execution_status:'completed',deterministic_pass:true,latency_ms:500,human_quality_score:5,quality_gate:gate,estimated_cost_per_1000_tasks:2,regressed_vs_champion:false}
]).model_id, 'champion');

// Cases group into their task classes.
const groups = groupByClass([
  {id:'A-1',task_class:'alpha',department:'D',team:'T',process:'P',business_value:'B',data_classification:'internal'},
  {id:'A-2',task_class:'alpha',department:'D',team:'T',process:'P',business_value:'B',data_classification:'internal'},
  {id:'B-1',task_class:'beta',department:'D',team:'T',process:'P',business_value:'B',data_classification:'internal'}
]);
assert.equal(groups.length, 2);
assert.equal(groups.find(g => g.task_class === 'alpha').cases.length, 2);

console.log('web policy tests passed');
