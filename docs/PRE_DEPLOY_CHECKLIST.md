# Pre-deployment checklist

Date: 23 July 2026
Scope: Tessera Model Gate POC
Test data: synthetic only

This checklist governs the move from a private build to a POC that real users
touch. It records what the control plane now enforces, how to configure each
control, and the human sign-offs that code cannot provide.

## 1. Choose the deployment path

The required controls depend on how users reach the service.

| Path | Description | Network | Minimum controls |
| --- | --- | --- | --- |
| A. Local demo | Each pilot user runs the service on their own workstation in demo mode on synthetic data. | Loopback only. | Sections 2.4 (demo isolation) already hold. Auth and encryption are optional. |
| B. Shared instance | Several users reach one instance over a network. | Bind beyond loopback. | Every control in section 2 is mandatory before the first shared request. |

Do not bind beyond loopback until section 2.1, 2.2, and 2.3 are green.

## 2. Technical controls

### 2.1 Authentication and access control — implemented
- Copy `config/access.example.yaml` to `config/access.yaml` and set real tokens.
- Roles: `viewer` (read), `reviewer` (read plus scoring), `operator` (read, scoring, run).
- When `config/access.yaml` is absent the API is open and warns at startup. This is only for path A on loopback.
- Generate tokens with `openssl rand -hex 24`. `config/access.yaml` is git-ignored.

### 2.2 Data-at-rest encryption — implemented
- Set `MODEL_GATE_DB_KEY` to a base64 32-byte key. Generate with `openssl rand -base64 32`.
- The evidence payload is sealed with AES-256-GCM before it reaches SQLite.
- Store the key in a secret manager, not in the repository or a shell history file.
- Rotating the key cannot read older records; export before a rotation if history must survive.

### 2.3 Retention and purge — implemented
- Set `--retention-days N` (or `MODEL_GATE_RETENTION_DAYS`). Records older than the window are purged at startup and hourly.
- Agree the retention period with legal and information security before a shared deploy.
- Unset means keep every record; do not run a shared deploy without an agreed period.

### 2.4 Demo and live isolation — implemented
- A demo run can only reach `demo_only`, never `eligible`.
- Live mode is selectable in the dashboard, defaults to demo, requires an operator token, and confirms the call count before it runs.
- A live run is refused unless the model is `approved_for_live` and counter-signed (section 2.7).

### 2.5 Rate and size limits — implemented
- `--rate-limit-per-minute` (default 240) caps API requests per minute; excess returns HTTP 429.
- `--max-body-bytes` (default 65536) rejects oversize request bodies.

### 2.6 Cost basis — implemented
- A model with a token price must record `cost_basis` describing how the price was set, or the service refuses to start.
- Unknown cost stays `null` and shows as "Not configured". Zero is never used for an unknown cost.
- Replace the illustrative hosted price with the contracted rate before any live run.

### 2.7 Signed model approvals — implemented
- `approved_for_live` in `config/models.yaml` is a request, not an authority.
- Copy `config/approvals.example.yaml` to `config/approvals.yaml`. A live model needs a matching record naming the approver and the exact artifact digest, or the service refuses to start.
- This enforces separation of duties: editing the catalogue alone cannot promote a model.

### 2.8 Champion, challenger, and variance — implemented
- `config/champions.yaml` names one champion per task class.
- Each run is tagged champion or challenger. A challenger that fails a case the champion passed is flagged `regressed_vs_champion` and can never be the selected route.
- Set `repetitions` (1, 3, or 5) to expose variance on stochastic runs.
- Promotion is deliberately manual: two consecutive passing runs and an owner's approval before the champion changes.

### 2.9 Benchmark depth — implemented
- Each task class now carries 15 synthetic test cases (60 in total), generated with a deterministic oracle by `scripts/generate_benchmarks.mjs`.
- CI fails if `config/benchmarks.yaml` drifts from the generator.
- Grow each pack toward 50 to 100 de-identified, approved cases before a production decision.

## 3. Governance sign-offs — required, not code

These gates are human decisions. Record the owner, date, and outcome before a
pilot with any non-synthetic data.

| Gate | Owner | Status | Evidence required |
| --- | --- | --- | --- |
| Legal and privacy (POPIA section 19) | Legal | Not started | Data-processing basis, retention period, cross-border position. |
| Information security | InfoSec | Not started | Key management, access review, network placement, logging review. |
| Model risk | Model Risk | Not started | Approved model list, champion policy, promotion rule sign-off. |
| Actuarial review | Actuarial | Not started | Confirmation that the tool informs but does not make an actuarial decision. |
| Data classification | Data Owner | Not started | Confirmed data classes per task class and approved routes. |

No pilot with de-identified or real data may start until every row is complete.

## 4. Deploy runbook (path B)

1. `cp config/access.example.yaml config/access.yaml` and set real tokens.
2. `cp config/approvals.example.yaml config/approvals.yaml` if any model is live-approved.
3. `export MODEL_GATE_DB_KEY="$(openssl rand -base64 32)"` and store it in the secret manager.
4. Review `config/champions.yaml`, `config/models.yaml`, and `config/benchmarks.yaml`.
5. `cargo test --all-targets` and the browser tests in the README pass.
6. Start with an agreed retention window and a non-loopback bind only behind a reverse proxy that terminates TLS:
   `./target/release/tessera-model-gate --bind 127.0.0.1:8080 --retention-days 30`
7. Confirm startup logs show access control enforced and encryption enabled.
8. Smoke test: no-token request returns 401; operator token can run; confidential previews are omitted.

## 5. Still out of scope for the POC

- Real client or personal data.
- A final actuarial, customer, claims, pricing, or conduct decision by the model.
- Automated champion promotion.
- A production security accreditation.

The POC informs a pilot decision. It does not approve a model for production.
