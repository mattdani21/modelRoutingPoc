# User and Chief Data Officer test report

Date: 21 July 2026  
Scope: Tessera Model Gate POC  
Test data: synthetic only

## 1. Release decision

The original build was not ready for a controlled pilot.
It had two Rust compile defects and several governance defects.
The defects in this report are fixed on the test branch.

The build remains a POC.
Do not connect real client data yet.
Add authentication before shared deployment.

## 2. User tests

| ID | Test | Original result | Severity | Fix |
| --- | --- | --- | --- | --- |
| U-01 | Load the task and model catalog. | The catalog loaded in standalone mode. | Low | Kept the recovery mode. Added a clear synthetic-demo label. |
| U-02 | Run a confidential task with all models. | The hosted model caused a request failure after some local results were stored. | Critical | The UI filters ineligible routes. The API validates the full request before any call or write. |
| U-03 | Select the best model after an automated pass. | The UI selected a winner before human review. | Critical | The decision stays at `pending_human_review` until an identified reviewer scores it. |
| U-04 | Score a result. | The score had no reviewer identity. A failed review request was not shown. | High | Reviewer identity is required. API failures are shown. The review time is stored. |
| U-05 | Use demo results. | Demo results looked like production evidence. | Critical | Every run has an execution mode. A demo can end only in `demo_only`, not `eligible`. |
| U-06 | Export the ledger. | The CSV omitted required business, model, runtime, hardware, and provenance fields. | High | The export now includes the complete decision record. |
| U-07 | View an unknown cost. | Local token cost appeared as R0.00. | High | Unknown cost is now `Not configured`. Zero is no longer used as an unknown value. |
| U-08 | Use the form with assistive technology. | Two labels were not connected to their controls. | Medium | Added explicit labels, score labels, and a live task-context region. |

## 3. Chief Data Officer tests

| ID | Control | Original result | Severity | Fix |
| --- | --- | --- | --- | --- |
| CDO-01 | Compile integrity | `RunResult` did not implement deserialization. `DataClassification` did not implement equality for route checks. | Critical | Added the required traits and CI compilation. |
| CDO-02 | Actuarial correctness | JSON grading checked field names only. Wrong values could pass. | Critical | Added exact JSON value grading for controlled outputs. |
| CDO-03 | Log precision | The grep test allowed extra false-positive lines. | Critical | Added exact line-set grading. Extra lines fail. |
| CDO-04 | Quality policy | YAML quality and latency gates were not used by routing. | Critical | One shared policy now enforces deterministic, human, latency, execution, and demo gates. |
| CDO-05 | Data minimisation | Confidential model output was stored in a response preview. | Critical | Confidential and restricted previews are omitted on write and read. |
| CDO-06 | Model provenance | The ledger did not record prompt version, catalog versions, licence, registry source, provider model ID, or artifact digest. | High | Added all fields to each run and the CSV export. |
| CDO-07 | Live approval | Any enabled model could run against its endpoint. | Critical | Live runs require `approved_for_live` and an artifact digest. All sample routes default to not approved. |
| CDO-08 | Endpoint assertion | A route marked `local` could use a remote URL and still claim data stayed local. | Critical | POC validation requires a loopback URL for local routes. The ledger now says the location is declared, not independently verified. |
| CDO-09 | Provider failure | One provider error ended the request and hid completed comparisons. | High | Provider failures become explicit rejected ledger results. The remaining comparisons continue. |
| CDO-10 | Duplicate work | Duplicate task or model IDs created duplicate charges and records. | High | Duplicate IDs are rejected before execution. |
| CDO-11 | API disclosure | The model catalog returned endpoint and API-key environment details. | Medium | These configuration fields are no longer serialized to the browser. |
| CDO-12 | CSV injection | Spreadsheet control characters were not neutralised. | Medium | Export values that start with formula characters are neutralised. |

## 4. Tests added

- Rust catalog validation test.
- Rust deterministic grader tests.
- Rust quality-gate tests.
- Rust cost tests.
- Rust duplicate-request test.
- JavaScript route-control test.
- JavaScript review-gate test.
- JavaScript demo-isolation test.
- JavaScript CSV-injection test.
- HTML accessibility contract test.
- GitHub Actions compile and test workflow.

## 5. Test limitations

This workspace did not permit a Rust toolchain installation.
The local Chromium download was also blocked.
JavaScript, YAML, HTTP, policy, and HTML contract tests ran locally.
Rust compilation and unit tests run in GitHub Actions.

No real model endpoint was used.
No real personal information was used.
No production control was tested.

## 6. Remaining pilot gates

- Add authentication and role-based access control.
- Add an approved retention period and purge job.
- Add signed model and benchmark approvals.
- Add encrypted database storage or an approved managed store.
- Add 15 reviewed cases for each selected task class.
- Add deterministic golden values from the EAC2 oracle.
- Add precision and recall measures for the Rust log tool.
- Measure hardware and energy cost for each local route.
- Complete legal, information security, model risk, and actuarial review.
