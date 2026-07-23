const fallbackTasks = [
  {id:'ACT-EAC2-001',prompt_version:'1.1',department:'Actuarial',team:'Product Calculations',business_value:'Prevent an incorrect EAC2 disclosure and reduce manual review time.',process:'Review an EAC2 extension scenario.',task_class:'controlled_reasoning',data_classification:'internal',quality_gate:{deterministic_pass_required:true,minimum_human_score:4,maximum_latency_ms:8000}},
  {id:'SYS-CTRL-001',prompt_version:'1.1',department:'Actuarial Systems',team:'Systems Control',business_value:'Find calculation differences before a release reaches production.',process:'Triage an ACE-to-MTS control difference.',task_class:'structured_triage',data_classification:'confidential',quality_gate:{deterministic_pass_required:true,minimum_human_score:4,maximum_latency_ms:5000}},
  {id:'QE-TEST-001',prompt_version:'1.0',department:'Actuarial Systems',team:'Test Analysis',business_value:'Reduce test design time and improve control coverage.',process:'Draft tests for an EAC2 rule change.',task_class:'test_design',data_classification:'internal',quality_gate:{deterministic_pass_required:true,minimum_human_score:4,maximum_latency_ms:8000}},
  {id:'LOG-GREP-001',prompt_version:'1.1',department:'Actuarial Systems',team:'Production Support',business_value:'Reduce log investigation time from minutes to seconds.',process:'Find the relevant policy errors in a service log.',task_class:'log_extraction',data_classification:'confidential',quality_gate:{deterministic_pass_required:true,minimum_human_score:4,maximum_latency_ms:3000}}
];
const fallbackModels = [
  {id:'qwen36-27b-q8',display_name:'Qwen3.6 27B Q8',provider:'local',model:'Qwen/Qwen3.6-27B',quantisation:'Q8',runtime:'llama.cpp',hardware:'32 GB sandbox',license:'Apache-2.0',registry_source:'Hugging Face',allowed_data:['public','internal','confidential','restricted']},
  {id:'gpt-oss-20b-local',display_name:'gpt-oss 20B',provider:'local',model:'openai/gpt-oss-20b',quantisation:'MXFP4',runtime:'vLLM',hardware:'Workstation',license:'Apache-2.0',registry_source:'Hugging Face',allowed_data:['public','internal','confidential','restricted']},
  {id:'deepseek-v4-flash',display_name:'DeepSeek V4 Flash',provider:'local',model:'deepseek-ai/DeepSeek-V4-Flash',quantisation:'native',runtime:'vLLM',hardware:'Server',license:'MIT',registry_source:'Hugging Face',allowed_data:['public','internal','confidential','restricted']},
  {id:'premium-frontier',display_name:'Premium frontier comparator',provider:'hosted',model:'frontier-comparator',quantisation:'managed',runtime:'Hosted API',hardware:'Provider managed',license:'Provider contract required',registry_source:'Approved company AI gateway',allowed_data:['public','internal']}
];
// Champion per task class for the standalone (offline) demo only. The API is
// the source of truth when it is available.
const fallbackChampions = {controlled_reasoning:'qwen36-27b-q8',structured_triage:'qwen36-27b-q8',test_design:'qwen36-27b-q8',log_extraction:'qwen36-27b-q8'};

const logic = ModelGateLogic;
let tasks = fallbackTasks;
let models = fallbackModels;
let classes = logic.groupByClass(fallbackTasks);
let champions = fallbackChampions;
let results = [];
let apiReady = false;
let authRequired = false;
let currentRole = null;
const taskSelect = document.querySelector('#task-select');
const tokenInput = document.querySelector('#token-input');

const storedToken = () => sessionStorage.getItem('mg_token') || '';
tokenInput.value = storedToken();
tokenInput.addEventListener('change', async () => {
  sessionStorage.setItem('mg_token', tokenInput.value.trim());
  await refreshSession();
});

function authHeaders(base) {
  const headers = Object.assign({}, base || {});
  const token = storedToken();
  if (token) headers['Authorization'] = `Bearer ${token}`;
  return headers;
}

function escapeHtml(value) {
  return String(value ?? '').replace(/[&<>'"]/g, character => ({'&':'&amp;','<':'&lt;','>':'&gt;',"'":'&#39;','"':'&quot;'}[character]));
}

async function refreshSession() {
  try {
    const response = await fetch('/api/session', {headers: authHeaders()});
    if (!response.ok) throw new Error('no session');
    const session = await response.json();
    authRequired = !!session.auth_required;
    currentRole = session.role;
    const status = document.querySelector('#auth-status');
    if (!authRequired) {
      status.innerHTML = '<span class="dot"></span> Open mode · loopback only';
    } else if (session.role) {
      status.innerHTML = `<span class="dot"></span> ${escapeHtml(session.principal || 'Signed in')} · ${escapeHtml(session.role)}`;
    } else {
      status.innerHTML = '<span class="dot"></span> Access token required';
    }
  } catch (_) {
    /* standalone mode: leave the default label */
  }
}

async function loadCatalogs() {
  await refreshSession();
  try {
    const [taskResponse, modelResponse, championResponse] = await Promise.all([
      fetch('/api/benchmarks', {headers: authHeaders()}),
      fetch('/api/models', {headers: authHeaders()}),
      fetch('/api/champions', {headers: authHeaders()})
    ]);
    if (!taskResponse.ok || !modelResponse.ok) throw new Error('API not ready');
    tasks = (await taskResponse.json()).tasks;
    models = (await modelResponse.json()).models.filter(model => model.enabled);
    classes = logic.groupByClass(tasks);
    champions = {};
    if (championResponse.ok) {
      for (const entry of (await championResponse.json()).champions || []) champions[entry.task_class] = entry.model_id;
    }
    apiReady = true;
    document.querySelector('#mode-badge').textContent = 'API · connected';
  } catch (_) {
    classes = logic.groupByClass(tasks);
    document.querySelector('#mode-badge').textContent = 'Standalone synthetic demo';
  }
  document.querySelector('#task-count').textContent = tasks.length;
  document.querySelector('#model-count').textContent = models.length;
  taskSelect.replaceChildren(...classes.map(group => {
    const option = document.createElement('option');
    option.value = group.task_class;
    option.textContent = `${group.department} · ${group.process} (${group.cases.length} cases)`;
    return option;
  }));
  updateTaskContext();
}

function selectedClass() {
  return classes.find(group => group.task_class === taskSelect.value) || classes[0];
}

function updateTaskContext() {
  const group = selectedClass();
  if (!group) return;
  const eligibleRoutes = models.filter(model => logic.isAllowedForTask(model, group)).length;
  const champion = champions[group.task_class];
  const championName = champion ? (models.find(m => m.id === champion)?.display_name || champion) : 'none registered';
  document.querySelector('#task-context').textContent =
    `${group.team} · ${group.business_value} · Data class: ${group.data_classification} · ${group.cases.length} test cases · Approved routes: ${eligibleRoutes} · Champion: ${championName}`;
}

function makeDemoResults(group, selectedModels, repetitions) {
  const evaluationId = crypto.randomUUID ? crypto.randomUUID() : `demo-${Date.now()}`;
  const rows = [];
  const championId = champions[group.task_class];
  for (const task of group.cases) {
    for (let index = 0; index < selectedModels.length; index++) {
      const model = selectedModels[index];
      for (let rep = 1; rep <= repetitions; rep++) {
        const pass = !(task.id === 'ACT-EAC2-001' && model.id === 'gpt-oss-20b-local');
        const tokensIn = 220 + task.id.length * 4;
        const tokensOut = 58 + index * 21;
        rows.push({
          run_id: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${index}-${rep}`,
          evaluation_id:evaluationId, created_at:new Date().toISOString(), execution_mode:'demo', execution_status:'completed',
          benchmark_version:'standalone-demo', model_catalog_version:'standalone-demo',
          task_id:task.id, prompt_version:task.prompt_version, department:group.department, team:group.team,
          business_value:group.business_value, process:group.process, task_class:group.task_class,
          model_id:model.id, provider_model_id:model.model, quantisation:model.quantisation, runtime:model.runtime,
          hardware:model.hardware, license:model.license, registry_source:model.registry_source, artifact_digest:null,
          deterministic_pass:pass, grader_detail:pass?'Synthetic deterministic pass':'Synthetic deterministic failure',
          quality_gate:task.quality_gate, human_quality_score:null, reviewer:null, reviewed_at:null,
          latency_ms:720 + index * 490 + task.id.length * 11, tokens_in:tokensIn, tokens_out:tokensOut,
          estimated_cost_per_1000_tasks:model.provider === 'hosted' ? ((tokensIn*2+tokensOut*10)/1000) : null,
          cost_basis:model.provider === 'hosted' ? 'Illustrative placeholder rate.' : null,
          repetition:rep, is_champion:model.id === championId, regressed_vs_champion:false,
          data_classification:group.data_classification,
          sovereignty_note:model.provider === 'local' ? 'Declared local route. Endpoint location is not independently verified.' : 'Hosted route. Contract and region check required.'
        });
      }
    }
  }
  // Flag challenger regressions against the champion within this evaluation.
  const championPassed = {};
  for (const row of rows) if (row.is_champion) championPassed[row.task_id] = (championPassed[row.task_id] ?? true) && row.deterministic_pass;
  for (const row of rows) if (!row.is_champion && championPassed[row.task_id] && !row.deterministic_pass) row.regressed_vs_champion = true;
  return rows;
}

async function runBenchmark() {
  const button = document.querySelector('#run-button');
  const group = selectedClass();
  const scope = document.querySelector('#model-select').value;
  const live = document.querySelector('#exec-mode').value === 'live';
  const repetitions = Number(document.querySelector('#repetitions').value) || 1;
  const selectedModels = models.filter(model => logic.isAllowedForTask(model, group) && (scope === 'all' || model.provider === 'local'));
  try {
    if (!group) throw new Error('Select a task class.');
    if (!selectedModels.length) throw new Error('No approved model route is available for this task class and data class.');
    if (live) {
      if (authRequired && currentRole !== 'operator') throw new Error('A live run requires an operator token.');
      const calls = group.cases.length * selectedModels.length * repetitions;
      if (!confirm(`Live mode calls real model endpoints ${calls} time(s) and may incur cost. A model must be approved for live. Continue?`)) return;
    }
    button.disabled = true;
    button.textContent = 'Running…';
    if (apiReady) {
      const response = await fetch('/api/runs', {method:'POST',headers:authHeaders({'Content-Type':'application/json'}),body:JSON.stringify({task_ids:group.cases.map(c=>c.id),model_ids:selectedModels.map(model=>model.id),demo:!live,repetitions})});
      if (!response.ok) throw new Error((await response.json()).error || 'Run failed');
      results = await response.json();
    } else {
      if (live) throw new Error('Live mode needs the Rust service. The standalone page only runs synthetic demo data.');
      await new Promise(resolve => setTimeout(resolve, 300));
      results = makeDemoResults(group, selectedModels, repetitions);
    }
    results.forEach(result => { result.gate_status = logic.gateStatus(result); });
    renderResults(group);
  } catch (error) {
    alert(error.message);
  } finally {
    button.disabled = false;
    button.innerHTML = 'Run benchmark <span>→</span>';
  }
}

function decisionLabel(status) {
  return status === 'eligible' ? 'Eligible' : status === 'demo_only' ? 'Demo only' : status === 'rejected' ? 'Rejected' : 'Awaiting review';
}

function renderResults(group) {
  const body = document.querySelector('#results-body');
  body.innerHTML = results.map(result => {
    const model = models.find(item => item.id === result.model_id) || {display_name:result.model_id};
    const status = logic.gateStatus(result);
    const latency = result.latency_ms == null ? 'Not reported' : `${Number(result.latency_ms).toLocaleString()} ms`;
    const tokens = result.tokens_in == null || result.tokens_out == null ? 'Not reported' : `${Number(result.tokens_in).toLocaleString()} / ${Number(result.tokens_out).toLocaleString()}`;
    const cost = result.estimated_cost_per_1000_tasks == null ? 'Not configured' : `R ${Number(result.estimated_cost_per_1000_tasks).toFixed(2)}`;
    const role = result.is_champion ? '<span class="champion">Champion</span>' : '<span class="challenger">Challenger</span>';
    const regressed = result.regressed_vs_champion ? '<br><span class="regressed">Regressed vs champion</span>' : '';
    const rep = Number(result.repetition || 1) > 1 || results.some(r => r.task_id === result.task_id && r.model_id === result.model_id && r.repetition > 1) ? `<br><small>rep ${escapeHtml(result.repetition || 1)}</small>` : '';
    return `<tr><td><small>${escapeHtml(result.task_id)}</small>${rep}</td>
      <td><b>${escapeHtml(model.display_name)}</b><br><small>${escapeHtml(model.quantisation || '')} · ${escapeHtml(model.runtime || '')}</small></td>
      <td>${role}</td>
      <td><span class="${result.deterministic_pass?'pass':'fail'}">${result.deterministic_pass?'Pass':'Fail'}</span></td>
      <td><span class="${status==='pending_human_review'?'pending':status==='demo_only'?'pending':status}">${decisionLabel(status)}</span>${regressed}</td>
      <td class="score" data-id="${escapeHtml(result.run_id)}">${[1,2,3,4,5].map(number=>`<button type="button" aria-label="Score ${number}" data-score="${number}" class="${result.human_quality_score===number?'active':''}">${number}</button>`).join('')}</td>
      <td>${escapeHtml(latency)}</td><td>${escapeHtml(tokens)}</td><td>${escapeHtml(cost)}</td><td>${escapeHtml(result.sovereignty_note)}</td></tr>`;
  }).join('');
  body.querySelectorAll('.score button').forEach(button => button.addEventListener('click', scoreResult));
  updateSummary(group);
}

function updateSummary(group) {
  const passed = results.filter(item => item.deterministic_pass);
  const best = logic.selectBestEligible(results);
  const pending = results.some(item => logic.gateStatus(item) === 'pending_human_review');
  const demoOnly = results.some(item => logic.gateStatus(item) === 'demo_only');
  document.querySelector('#pass-rate').textContent = results.length ? `${Math.round(passed.length/results.length*100)}%` : '—';
  document.querySelector('#best-model').textContent = best ? (models.find(model=>model.id===best.model_id)?.display_name || best.model_id) : pending ? 'Awaiting review' : demoOnly ? 'Demo only' : 'No eligible route';
  const latencies = results.map(item=>item.latency_ms).filter(value=>value != null);
  document.querySelector('#fastest').textContent = latencies.length ? `${Math.min(...latencies).toLocaleString()} ms` : 'Not reported';
  document.querySelector('#data-control').textContent = group.data_classification;
}

async function scoreResult(event) {
  const reviewer = document.querySelector('#reviewer-input').value.trim();
  if (!reviewer) {
    alert('Enter a reviewer name or staff ID before you record a human score.');
    document.querySelector('#reviewer-input').focus();
    return;
  }
  const cell = event.target.closest('.score');
  const score = Number(event.target.dataset.score);
  const result = results.find(item => item.run_id === cell.dataset.id);
  try {
    if (apiReady) {
      const response = await fetch(`/api/runs/${cell.dataset.id}/review`,{method:'POST',headers:authHeaders({'Content-Type':'application/json'}),body:JSON.stringify({score,reviewer})});
      if (!response.ok) throw new Error((await response.json()).error || 'Review failed');
    }
    result.human_quality_score = score;
    result.reviewer = reviewer;
    result.reviewed_at = new Date().toISOString();
    result.gate_status = logic.gateStatus(result);
    renderResults(selectedClass());
  } catch (error) {
    alert(error.message);
  }
}

function exportCsv() {
  if (!results.length) return;
  const fields = [
    'evaluation_id','run_id','created_at','execution_mode','execution_status','benchmark_version','model_catalog_version',
    'task_id','prompt_version','department','team','business_value','process','task_class','model_id','provider_model_id',
    'quantisation','runtime','hardware','license','registry_source','artifact_digest','deterministic_pass','gate_status',
    'is_champion','regressed_vs_champion','repetition','human_quality_score','reviewer','reviewed_at','latency_ms','tokens_in','tokens_out',
    'estimated_cost_per_1000_tasks','cost_basis','data_classification','sovereignty_note'
  ];
  const csv = [fields.join(','), ...results.map(row => fields.map(field => logic.csvCell(field === 'gate_status' ? logic.gateStatus(row) : row[field])).join(','))].join('\n');
  const link = document.createElement('a');
  link.href = URL.createObjectURL(new Blob([csv],{type:'text/csv'}));
  link.download = `tessera-model-gate-${Date.now()}.csv`;
  link.click();
  URL.revokeObjectURL(link.href);
}

taskSelect.addEventListener('change', updateTaskContext);
document.querySelector('#run-button').addEventListener('click', runBenchmark);
document.querySelector('#export-button').addEventListener('click', exportCsv);
loadCatalogs();
