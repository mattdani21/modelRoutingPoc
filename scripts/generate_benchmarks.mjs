// Generate config/benchmarks.yaml with 15 synthetic test cases per task class.
//
// Every case carries a deterministic expected result computed here, so the
// graders have a correct oracle without any hand-entered answer keys. The data
// is synthetic. Run: node scripts/generate_benchmarks.mjs
//
// Design note: the plan defines Task class -> Test case as separate levels.
// Each case is emitted as its own task entry sharing the class metadata and
// carrying its own prompt, grader oracle, and quality gate. The dashboard groups
// these cases by task_class.

import { writeFileSync } from "node:fs";

const VERSION = "2026-07-23.1";
const CASES_PER_CLASS = 15;

function q(text) {
  return `"${String(text).replaceAll("\\", "\\\\").replaceAll('"', '\\"')}"`;
}

function block(text, indent) {
  const pad = " ".repeat(indent);
  return text
    .trimEnd()
    .split("\n")
    .map((line) => (line.length ? pad + line : ""))
    .join("\n");
}

function round2(value) {
  return Math.round(value * 100) / 100;
}

// --- Class 1: controlled_reasoning (EAC2 total term disclosure) -------------
function eac2Cases() {
  const out = [];
  for (let i = 1; i <= CASES_PER_CLASS; i++) {
    const base = 10 + (i % 6) * 5; // 10..35
    const extension = 3 + (i % 4) * 2; // 3..9
    const correctTotal = base + extension;
    // Every fourth case discloses the correct total; the rest understate it.
    const mismatch = i % 4 !== 0;
    const disclosed = mismatch ? base : correctTotal;
    const expected = mismatch
      ? { risk: "high", finding_code: "DISCLOSED_TERM_MISMATCH", required_action_code: "CORRECT_TOTAL_TERM_BEFORE_ISSUE" }
      : { risk: "low", finding_code: "TERM_MATCH", required_action_code: "NONE" };
    const prompt = `Review this synthetic EAC2 case.
The base term is ${base} years.
The extension term is ${extension} years.
The disclosed total term is ${disclosed} years.
Return this JSON shape with no extra fields:
{"risk":"high|low","finding_code":"DISCLOSED_TERM_MISMATCH|TERM_MATCH","required_action_code":"CORRECT_TOTAL_TERM_BEFORE_ISSUE|NONE"}`;
    out.push({
      id: `ACT-EAC2-${String(i).padStart(3, "0")}`,
      prompt,
      grader: [
        "    grader:",
        "      type: json_equals",
        "      expected:",
        `        risk: ${expected.risk}`,
        `        finding_code: ${expected.finding_code}`,
        `        required_action_code: ${expected.required_action_code}`,
      ].join("\n"),
    });
  }
  return {
    prompt_version: "1.1",
    department: "Actuarial",
    team: "Product Calculations",
    business_value: "Prevent an incorrect EAC2 disclosure and reduce manual review time.",
    process: "Review an EAC2 extension scenario.",
    task_class: "controlled_reasoning",
    data_classification: "internal",
    quality_gate: { deterministic_pass_required: true, minimum_human_score: 4, maximum_latency_ms: 8000 },
    cases: out,
  };
}

// --- Class 2: structured_triage (ACE vs MTS control difference) -------------
function ctrlCases() {
  const out = [];
  for (let i = 1; i <= CASES_PER_CLASS; i++) {
    const mts = 100 + i; // 101..115
    const tolerance = 2.0;
    // Alternate inside and outside tolerance.
    const outside = i % 2 === 1;
    const diffPercent = outside ? 2.0 + (i % 3) * 0.4 + 0.2 : 0.5 + (i % 3) * 0.3;
    const ace = round2(mts * (1 + diffPercent / 100));
    const relative = round2((Math.abs(ace - mts) / mts) * 100);
    const status = relative > tolerance ? "outside_tolerance" : "within_tolerance";
    const priority = status === "outside_tolerance" ? "high" : "normal";
    const prompt = `Review this synthetic control result.
ACE value: ${ace}.
MTS value: ${mts}.
The allowed relative difference is ${tolerance.toFixed(2)} percent.
Calculate the absolute relative difference against MTS.
Return this JSON shape with no extra fields:
{"relative_difference_percent":0.0,"status":"within_tolerance|outside_tolerance","investigation_priority":"normal|high"}`;
    out.push({
      id: `SYS-CTRL-${String(i).padStart(3, "0")}`,
      prompt,
      grader: [
        "    grader:",
        "      type: json_equals",
        "      expected:",
        `        relative_difference_percent: ${relative}`,
        `        status: ${status}`,
        `        investigation_priority: ${priority}`,
      ].join("\n"),
    });
  }
  return {
    prompt_version: "1.1",
    department: "Actuarial Systems",
    team: "Systems Control",
    business_value: "Find calculation differences before a release reaches production.",
    process: "Triage an ACE-to-MTS control difference.",
    task_class: "structured_triage",
    data_classification: "confidential",
    quality_gate: { deterministic_pass_required: true, minimum_human_score: 4, maximum_latency_ms: 5000 },
    cases: out,
  };
}

// --- Class 3: test_design (draft a compact test plan) -----------------------
function testCases() {
  const rules = [
    "A policy can have a base term and one extension term. The EAC2 letter must show the total term.",
    "A premium waiver applies only while a disability claim is active.",
    "A paid-up value must never exceed the sum assured.",
    "A loan balance plus interest must not exceed the surrender value.",
    "A maturity payout must include all vested bonuses.",
  ];
  const out = [];
  for (let i = 1; i <= CASES_PER_CLASS; i++) {
    const rule = rules[i % rules.length];
    const prompt = `Draft a compact test plan for this synthetic rule.
${rule}
Return one JSON object.
Use these fields: happy_path, boundary_case, negative_case, expected_control.`;
    out.push({
      id: `QE-TEST-${String(i).padStart(3, "0")}`,
      prompt,
      grader: [
        "    grader:",
        "      type: json_fields",
        "      fields: [happy_path, boundary_case, negative_case, expected_control]",
      ].join("\n"),
    });
  }
  return {
    prompt_version: "1.0",
    department: "Actuarial Systems",
    team: "Test Analysis",
    business_value: "Reduce test design time and improve control coverage.",
    process: "Draft tests for an EAC2 rule change.",
    task_class: "test_design",
    data_classification: "internal",
    quality_gate: { deterministic_pass_required: true, minimum_human_score: 4, maximum_latency_ms: 8000 },
    cases: out,
  };
}

// --- Class 4: log_extraction (return the matching ERROR lines) --------------
function logCases() {
  const sources = ["ACE", "MTS", "ROUTER", "GATEWAY"];
  const out = [];
  for (let i = 1; i <= CASES_PER_CLASS; i++) {
    const target = `POL${100 + i}`;
    const other = `POL${900 + i}`;
    const codeA = `E4${String(i).padStart(2, "0")}`;
    const codeB = `E5${String(i).padStart(2, "0")}`;
    const srcA = sources[i % sources.length];
    const srcB = sources[(i + 1) % sources.length];
    const errorA = `ERROR request=1 policy=${target} code=${codeA} source=${srcA}`;
    const errorB = `ERROR request=1 policy=${target} code=${codeB} source=${srcB}`;
    const logLines = [
      `INFO request=1 policy=${target} status=started`,
      errorA,
      `ERROR request=2 policy=${other} code=E500 source=MTS`,
      `WARN request=1 policy=${target} retry=true`,
      errorB,
    ];
    const prompt = `Read the synthetic log lines.
Return only the two ERROR lines that contain policy ${target}.
Keep their original order.

${logLines.join("\n")}`;
    out.push({
      id: `LOG-GREP-${String(i).padStart(3, "0")}`,
      prompt,
      grader: [
        "    grader:",
        "      type: exact_lines",
        "      ignore_order: false",
        "      expected:",
        `        - ${q(errorA)}`,
        `        - ${q(errorB)}`,
      ].join("\n"),
    });
  }
  return {
    prompt_version: "1.1",
    department: "Actuarial Systems",
    team: "Production Support",
    business_value: "Reduce log investigation time from minutes to seconds.",
    process: "Find the relevant policy errors in a service log.",
    task_class: "log_extraction",
    data_classification: "confidential",
    quality_gate: { deterministic_pass_required: true, minimum_human_score: 4, maximum_latency_ms: 3000 },
    cases: out,
  };
}

function emit(classes) {
  const lines = [
    "# Generated by scripts/generate_benchmarks.mjs. Do not edit by hand.",
    "# Synthetic data only. Each task is one reviewed test case within a task class.",
    `version: ${q(VERSION)}`,
    "tasks:",
  ];
  for (const klass of classes) {
    for (const testCase of klass.cases) {
      lines.push(`  - id: ${testCase.id}`);
      lines.push(`    prompt_version: ${q(klass.prompt_version)}`);
      lines.push(`    department: ${q(klass.department)}`);
      lines.push(`    team: ${q(klass.team)}`);
      lines.push(`    business_value: ${q(klass.business_value)}`);
      lines.push(`    process: ${q(klass.process)}`);
      lines.push(`    task_class: ${klass.task_class}`);
      lines.push(`    data_classification: ${klass.data_classification}`);
      lines.push("    prompt: |");
      lines.push(block(testCase.prompt, 6));
      lines.push(testCase.grader);
      lines.push("    quality_gate:");
      lines.push(`      deterministic_pass_required: ${klass.quality_gate.deterministic_pass_required}`);
      lines.push(`      minimum_human_score: ${klass.quality_gate.minimum_human_score}`);
      lines.push(`      maximum_latency_ms: ${klass.quality_gate.maximum_latency_ms}`);
      lines.push("");
    }
  }
  return lines.join("\n");
}

const classes = [eac2Cases(), ctrlCases(), testCases(), logCases()];
const total = classes.reduce((sum, klass) => sum + klass.cases.length, 0);
writeFileSync("config/benchmarks.yaml", emit(classes) + "\n");
console.log(`Wrote config/benchmarks.yaml with ${classes.length} task classes and ${total} test cases.`);
