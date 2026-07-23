const assert = require('node:assert/strict');
const { isAllowedForTask, gateStatus, selectBestEligible, csvCell } = require('../web/logic.js');

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

console.log('web policy tests passed');
