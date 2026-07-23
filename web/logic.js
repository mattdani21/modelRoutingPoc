(function (root, factory) {
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  root.ModelGateLogic = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  function isAllowedForTask(model, task) {
    return Array.isArray(model.allowed_data) && model.allowed_data.includes(task.data_classification);
  }

  function gateStatus(result) {
    if (result.execution_status && result.execution_status !== 'completed') return 'rejected';
    const gate = result.quality_gate || {};
    if (gate.deterministic_pass_required !== false && !result.deterministic_pass) return 'rejected';
    if (gate.maximum_latency_ms != null && (result.latency_ms == null || result.latency_ms > gate.maximum_latency_ms)) return 'rejected';
    if (gate.minimum_human_score != null) {
      if (result.human_quality_score == null) return 'pending_human_review';
      if (result.human_quality_score < gate.minimum_human_score) return 'rejected';
      return result.execution_mode === 'demo' ? 'demo_only' : 'eligible';
    }
    return result.execution_mode === 'demo' ? 'demo_only' : 'eligible';
  }

  function selectBestEligible(results) {
    return results
      // A regression against the champion blocks promotion regardless of cost.
      .filter(result => gateStatus(result) === 'eligible' && !result.regressed_vs_champion)
      .slice()
      .sort((a, b) => {
        const costA = a.estimated_cost_per_1000_tasks == null ? Number.POSITIVE_INFINITY : a.estimated_cost_per_1000_tasks;
        const costB = b.estimated_cost_per_1000_tasks == null ? Number.POSITIVE_INFINITY : b.estimated_cost_per_1000_tasks;
        return costA - costB || (a.latency_ms ?? Number.POSITIVE_INFINITY) - (b.latency_ms ?? Number.POSITIVE_INFINITY);
      })[0] || null;
  }

  // Group task entries into their task classes. Each class shares department,
  // team, process, and data class metadata across its test cases.
  function groupByClass(tasks) {
    const groups = new Map();
    for (const task of tasks) {
      const key = task.task_class;
      if (!groups.has(key)) {
        groups.set(key, {
          task_class: key,
          department: task.department,
          team: task.team,
          process: task.process,
          business_value: task.business_value,
          data_classification: task.data_classification,
          cases: [],
        });
      }
      groups.get(key).cases.push(task);
    }
    return [...groups.values()];
  }

  function csvCell(value) {
    let text = String(value ?? '');
    if (/^[=+\-@\t\r]/.test(text)) text = `'${text}`;
    return `"${text.replaceAll('"', '""')}"`;
  }

  return { isAllowedForTask, gateStatus, selectBestEligible, csvCell, groupByClass };
});
