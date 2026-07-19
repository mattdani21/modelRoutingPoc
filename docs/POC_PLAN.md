# Tessera Model Gate POC plan

Date: 19 July 2026  
Pitch date: within three days  
Status: first working build

## 1. Decision request

Ask for approval to run a controlled six-week pilot.
Ask for one technical owner.
Ask for one risk and compliance owner.
Ask each selected team for one subject matter expert.
Ask for access to one approved workstation or GPU environment.

Do not ask for a large model purchase in this pitch.
Ask for approval to measure the need first.

## 2. Problem statement

The company can pay for a model before it knows if the model is fit for its work.
Public benchmarks do not measure the company process.
A model update can change quality without a process change.
Sensitive data can move to a route that is not approved.
The company needs a repeatable model acceptance gate.

## 3. POC outcome

The POC will show one control plane for several departments.
The POC will compare local and hosted models on the same task cases.
The POC will keep the task, model, hardware, quality, latency, cost, and data record.
The POC will select an eligible route for each task class.

## 4. Correct benchmark structure

Keep the fields that were proposed.
Use them as portfolio metadata.
Do not treat one row as a complete benchmark.

Use this structure:

| Level | Purpose | Example |
| --- | --- | --- |
| Portfolio | Report value across the company. | Actuarial Systems |
| Use case | Define the process and owner. | Log investigation |
| Task class | Define a route and a quality gate. | Log extraction |
| Test case | Give one input and expected result. | Find two policy errors |
| Run | Record one model attempt. | Qwen3.6 27B Q8 on case 014 |

The fields `department`, `team`, `business_value`, and `process` stay on each task.
Add a stable task ID and task class.
Add the data class.
Add a grader and quality gate.

This design prevents a popular model from winning because it is strong on an unrelated public test.

## 5. Benchmark method

### 5.1 Build task packs from real work

Use synthetic data for the POC.
Use de-identified and approved data only after control approval.
Start with 15 reviewed cases for each task class.
Grow each pack to 50 to 100 cases before a production decision.

Include normal cases.
Include boundary cases.
Include known failure cases.
Include cases where the correct action is to stop and ask for review.

### 5.2 Use several grader types

Use exact match for fixed outputs.
Use schema checks for structured outputs.
Use code or calculation tests for executable work.
Use expert review for judgement and explanation.
Use a model grader only when a deterministic grader is not enough.
Calibrate a model grader against expert scores before use.

Do not let a model grade itself.

### 5.3 Control test variance

Fix the prompt version.
Fix the model ID.
Fix the quantisation.
Fix the runtime and hardware.
Set temperature to zero for the first comparison.
Run each stochastic case at least three times.
Report the mean and the worst result.
Keep a hidden holdout set.

### 5.4 Use a promotion gate

A candidate becomes eligible only when it meets all mandatory gates.

| Gate | POC rule |
| --- | --- |
| Data route | The route must allow the task data class. |
| Correctness | All critical deterministic tests must pass. |
| Expert quality | The median score must be at least 4 of 5. |
| Regression | No critical case can regress from the current champion. |
| Latency | The result must meet the task limit. |
| Stability | The model must meet the gate on repeated runs. |

Select the lowest total cost only after these gates pass.
Use a frontier hosted model as a ceiling comparator.
Do not make it the default route.

### 5.5 Keep a champion and challenger

Keep the approved model as the champion.
Test a new model as the challenger.
Do not replace the champion from one run.
Require two consecutive benchmark runs.
Require an owner to approve the promotion.
Keep the prior route for rollback.

### 5.6 Re-test on events

Run the pack before a model change.
Run the pack before a quantisation change.
Run the pack before a prompt change.
Run the pack before a runtime change.
Run the pack each month during the pilot.
Run it each quarter after the route is stable.

## 6. Initial task packs

| Department | Team | Business value | Process | First POC measure |
| --- | --- | --- | --- | --- |
| Actuarial | Product Calculations | Prevent an incorrect EAC2 disclosure. | Review an EAC2 extension scenario. | Schema pass and expert correctness score. |
| Actuarial Systems | Systems Control | Find calculation differences before release. | Triage an ACE-to-MTS difference. | Correct threshold result and priority. |
| Actuarial Systems | Test Analysis | Reduce test design time. | Draft tests for an EAC2 rule. | Coverage, correctness, and edit time. |
| Actuarial Systems | Production Support | Reduce log investigation time. | Find policy errors in logs. | Precision, recall, and elapsed time. |

For the log use case, compare two solutions.
Compare an LLM-assisted tool with a deterministic Rust search tool.
Do not use an LLM when a normal search gives the correct answer at lower cost and risk.
Use the LLM for query translation, explanation, and investigation summaries.

## 7. Model shortlist review

The model market changes fast.
Use this list for discovery only.
Record the exact repository and commit before a run.

| Tier | Candidate | POC position |
| --- | --- | --- |
| Laptop or sandbox | Qwen3.6 27B with a measured quantisation | Test fit on the target machine. Do not assume that Q8 fits in 16 GB. |
| Laptop or workstation | gpt-oss 20B | Good local comparator. The reference model uses an Apache 2.0 licence. |
| Laptop or workstation | Gemma 4 31B with an approved quantisation | Check the Gemma licence and company approval. Do not call it Apache or MIT. |
| Workstation or server | Mistral Small 4 119B | Move it out of the laptop tier. It has 119B total parameters. |
| Server | DeepSeek V4 Flash | Treat it as a multi-GPU or server candidate. It has 284B total and about 13B active parameters. |
| Team serving | GLM-5.2 and other large open-weight candidates | Test only after infrastructure and licence review. |

Use `llama.cpp` or Ollama for a simple sandbox.
Use vLLM for a shared GPU service.
Use SGLang when measured prefix reuse or agent work justifies it.

All three runtimes can sit behind an OpenAI-compatible route.
Compatibility does not make every request or response identical.
Keep provider adapters and conformance tests.

Do not use SWE-bench as the purchase gate.
Use it as one discovery signal for coding work.
The company task packs remain the acceptance gate.

## 8. Evidence ledger

The first build records these fields:

- Task ID.
- Department.
- Team.
- Business value.
- Process.
- Task class.
- Model ID.
- Quantisation.
- Runtime.
- Hardware.
- Deterministic pass or fail.
- Grader detail.
- Human quality score from 1 to 5.
- Latency.
- Input and output tokens.
- Estimated cost per 1,000 tasks.
- Data classification.
- Sovereignty note.
- Response preview.

Add these fields in the pilot:

- Prompt version.
- Model file hash or provider version.
- Benchmark pack commit.
- Repetition number.
- Energy estimate.
- Reviewer ID.
- Approval state.
- Failure category.
- Estimated staff minutes saved.
- Total cost of ownership.

## 9. Data and regulatory controls

Use data minimisation.
Use synthetic data in the pitch.
Do not put personal information in logs or result previews.
Encrypt data in transit and at rest.
Restrict access by role.
Keep an audit record.
Set a retention period.
Complete legal, information security, model risk, and actuarial review before a pilot with real data.

POPIA section 19 requires reasonable technical and organisational measures for integrity and confidentiality.
Local hosting can support this control.
Local hosting does not create compliance by itself.

Do not let the model make a final actuarial, customer, claims, pricing, or conduct decision in the POC.

## 10. Three-day delivery plan

### Day 1: Build the proof

- Complete the task and model registries.
- Complete the Rust API.
- Complete the SQLite evidence ledger.
- Complete the standalone dashboard.
- Use synthetic EAC2, control, test, and log cases.

Exit condition: The dashboard can run all four tasks in demo mode.

### Day 2: Add evidence

- Connect one small local model.
- Connect one approved hosted comparator.
- Add 10 to 15 cases for the log task.
- Add 10 to 15 cases for one actuarial systems task.
- Ask two experts to score five outputs.
- Record latency and cost.

Exit condition: The pitch includes measured results from at least two routes.

### Day 3: Rehearse the decision

- Run the full POC pack twice.
- Freeze the task pack version.
- Export the evidence ledger.
- Record a two-minute backup demo.
- Rehearse a seven-minute pitch.
- Prepare the six-week pilot request.

Exit condition: The pitch can continue if the live model endpoint fails.

## 11. Seven-minute pitch

### Minute 0 to 1

State the risk.
The company must not buy model claims.
It must buy measured outcomes on its own work.

### Minute 1 to 2

Show the four business processes.
Show that one control plane serves more than one team.

### Minute 2 to 5

Run the log task.
Show the pass gate.
Add a human score.
Show latency, cost, and the data note.
Change the model route.

### Minute 5 to 6

Show the promotion rule.
Quality and data controls come before cost.

### Minute 6 to 7

Ask for the six-week pilot.
Ask for named owners and approved compute.
Commit to a go or no-go evidence report.

## 12. Pilot success measures

- At least two task classes have an eligible local route.
- The local route cuts model cost without a critical quality regression.
- The log task cuts median investigation time by at least 50 percent.
- Test analysts cut first-draft time by at least 30 percent.
- Every run has a complete evidence record.
- No unapproved data route occurs.
- Experts agree with the automated gate in at least 90 percent of reviewed cases.

## 13. Sources used for the POC

- OpenAI Evals: https://github.com/openai/evals
- Qwen3.6 27B model card: https://huggingface.co/Qwen/Qwen3.6-27B
- gpt-oss 20B model card: https://huggingface.co/openai/gpt-oss-20b
- Gemma 4 31B model card: https://huggingface.co/google/gemma-4-31B-it
- Mistral Small 4 model card: https://huggingface.co/mistralai/Mistral-Small-4-119B-2603
- DeepSeek V4 Flash model card: https://huggingface.co/deepseek-ai/DeepSeek-V4-Flash
- vLLM documentation: https://docs.vllm.ai/
- POPIA: https://www.gov.za/documents/protection-personal-information-act
- FSCA AI page: https://www2.fsca.co.za/Regulatory%20Frameworks/Pages/Artificial-Intelligence.aspx

Verify all model IDs, licences, prices, and hardware requirements again before the pilot starts.
