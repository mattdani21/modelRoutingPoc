const fallbackTasks = [
  {id:'ACT-EAC2-001',department:'Actuarial',team:'Product Calculations',process:'Review an EAC2 extension scenario.',task_class:'controlled_reasoning',data_classification:'internal'},
  {id:'SYS-CTRL-001',department:'Actuarial Systems',team:'Systems Control',process:'Triage an ACE-to-MTS control difference.',task_class:'structured_triage',data_classification:'confidential'},
  {id:'QE-TEST-001',department:'Actuarial Systems',team:'Test Analysis',process:'Draft tests for an EAC2 rule change.',task_class:'test_design',data_classification:'internal'},
  {id:'LOG-GREP-001',department:'Actuarial Systems',team:'Production Support',process:'Find the relevant policy errors in a service log.',task_class:'log_extraction',data_classification:'confidential'}
];
const fallbackModels = [
  {id:'qwen36-27b-q8',display_name:'Qwen3.6 27B Q8',provider:'local',quantisation:'Q8',runtime:'llama.cpp',hardware:'32 GB sandbox'},
  {id:'gpt-oss-20b-local',display_name:'gpt-oss 20B',provider:'local',quantisation:'MXFP4',runtime:'vLLM',hardware:'Workstation'},
  {id:'deepseek-v4-flash',display_name:'DeepSeek V4 Flash',provider:'local',quantisation:'native',runtime:'vLLM',hardware:'Server'},
  {id:'premium-frontier',display_name:'Premium frontier comparator',provider:'hosted',quantisation:'managed',runtime:'Hosted API',hardware:'Provider managed'}
];
let tasks = fallbackTasks, models = fallbackModels, results = [], apiReady = false;
const taskSelect = document.querySelector('#task-select');

async function loadCatalogs() {
  try {
    const [taskResponse, modelResponse] = await Promise.all([fetch('/api/benchmarks'), fetch('/api/models')]);
    if (!taskResponse.ok || !modelResponse.ok) throw new Error('API not ready');
    tasks = (await taskResponse.json()).tasks;
    models = (await modelResponse.json()).models.filter(model => model.enabled);
    apiReady = true;
    document.querySelector('#mode-badge').textContent = 'Live control plane';
  } catch (_) {
    document.querySelector('#mode-badge').textContent = 'Standalone demo';
  }
  document.querySelector('#task-count').textContent = tasks.length;
  document.querySelector('#model-count').textContent = models.length;
  taskSelect.innerHTML = tasks.map(task => `<option value="${task.id}">${task.department} · ${task.process}</option>`).join('');
}

function makeDemoResults(task, selectedModels) {
  return selectedModels.map((model, index) => {
    const pass = !(task.id === 'ACT-EAC2-001' && model.id === 'gpt-oss-20b-local');
    const tokensIn = 220 + task.id.length * 4;
    const tokensOut = 58 + index * 21;
    return {
      run_id: crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${index}`,
      task_id: task.id, model_id:model.id, task_class:task.task_class,
      deterministic_pass:pass, human_quality_score:null,
      latency_ms:720 + index * 490 + task.id.length * 11,
      tokens_in:tokensIn, tokens_out:tokensOut,
      estimated_cost_per_1000_tasks:model.provider === 'hosted' ? ((tokensIn*2+tokensOut*10)/1000) : 0,
      data_classification:task.data_classification,
      sovereignty_note:model.provider === 'local' ? 'Data stays in the approved local environment' : 'Contract and region check required'
    };
  });
}

async function runBenchmark() {
  const button = document.querySelector('#run-button'); button.disabled = true; button.textContent = 'Running…';
  const task = tasks.find(item => item.id === taskSelect.value);
  const scope = document.querySelector('#model-select').value;
  const selectedModels = models.filter(model => scope === 'all' || model.provider === 'local');
  try {
    if (apiReady) {
      const response = await fetch('/api/runs', {method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({task_ids:[task.id],model_ids:selectedModels.map(model=>model.id),demo:true})});
      if (!response.ok) throw new Error((await response.json()).error || 'Run failed');
      results = await response.json();
    } else {
      await new Promise(resolve => setTimeout(resolve, 700));
      results = makeDemoResults(task, selectedModels);
    }
    renderResults(task);
  } catch (error) { alert(error.message); }
  finally { button.disabled = false; button.innerHTML = 'Run benchmark <span>→</span>'; }
}

function renderResults(task) {
  const body = document.querySelector('#results-body');
  body.innerHTML = results.map(result => {
    const model = models.find(item => item.id === result.model_id) || {display_name:result.model_id};
    return `<tr><td><b>${model.display_name}</b><br><small>${model.quantisation || ''} · ${model.runtime || ''}</small></td>
      <td><span class="${result.deterministic_pass?'pass':'fail'}">${result.deterministic_pass?'Pass':'Fail'}</span></td>
      <td class="score" data-id="${result.run_id}">${[1,2,3,4,5].map(n=>`<button data-score="${n}">${n}</button>`).join('')}</td>
      <td>${result.latency_ms.toLocaleString()} ms</td><td>${result.tokens_in.toLocaleString()} / ${result.tokens_out.toLocaleString()}</td>
      <td>R ${result.estimated_cost_per_1000_tasks.toFixed(2)}</td><td>${result.sovereignty_note}</td></tr>`;
  }).join('');
  body.querySelectorAll('.score button').forEach(button => button.addEventListener('click', scoreResult));
  const passed = results.filter(item => item.deterministic_pass);
  document.querySelector('#pass-rate').textContent = `${Math.round(passed.length/results.length*100)}%`;
  document.querySelector('#best-model').textContent = passed.length ? (models.find(model=>model.id===passed.sort((a,b)=>a.estimated_cost_per_1000_tasks-b.estimated_cost_per_1000_tasks||a.latency_ms-b.latency_ms)[0].model_id)?.display_name || '—') : 'No route';
  document.querySelector('#fastest').textContent = `${Math.min(...results.map(item=>item.latency_ms)).toLocaleString()} ms`;
  document.querySelector('#data-control').textContent = task.data_classification;
}

async function scoreResult(event) {
  const cell = event.target.closest('.score'), score = Number(event.target.dataset.score);
  cell.querySelectorAll('button').forEach(button => button.classList.toggle('active',Number(button.dataset.score)===score));
  const result = results.find(item => item.run_id === cell.dataset.id); if (result) result.human_quality_score = score;
  if (apiReady) await fetch(`/api/runs/${cell.dataset.id}/review`,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({score})});
}

function exportCsv() {
  if (!results.length) return;
  const fields=['task_id','model_id','deterministic_pass','human_quality_score','latency_ms','tokens_in','tokens_out','estimated_cost_per_1000_tasks','data_classification','sovereignty_note'];
  const escape=value=>`"${String(value??'').replaceAll('"','""')}"`;
  const csv=[fields.join(','),...results.map(row=>fields.map(field=>escape(row[field])).join(','))].join('\n');
  const link=document.createElement('a'); link.href=URL.createObjectURL(new Blob([csv],{type:'text/csv'})); link.download=`tessera-model-gate-${Date.now()}.csv`; link.click(); URL.revokeObjectURL(link.href);
}

document.querySelector('#run-button').addEventListener('click',runBenchmark);
document.querySelector('#export-button').addEventListener('click',exportCsv);
loadCatalogs();
