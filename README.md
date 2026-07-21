# Tessera Model Gate

Tessera Model Gate compares language models on approved business tasks.
It blocks a model when the model does not meet a task gate.
It stores the evidence for each decision.

## POC scope

The POC includes these functions:

- Load versioned benchmark tasks from YAML.
- Load model routes from YAML.
- call an OpenAI-compatible model endpoint.
- Run a safe demo without a model endpoint.
- Apply deterministic graders.
- Capture a human quality score from 1 to 5.
- Store results in SQLite.
- Export the evidence ledger as CSV.
- Block a route that is not approved for the task data class.
- Block a live run until the model has an approval flag and artifact digest.
- Keep demo evidence separate from live evidence.
- Require an identified human reviewer before a quality decision.

The POC does not approve a model for production.
The POC does not send real client data.
The POC does not replace actuarial review.
The POC binds to loopback by default.
Do not bind it to a shared interface until authentication and access controls are added.

## Start the service

Install a current stable Rust toolchain.
Then run these commands from this directory.

```bash
cargo test
cargo run
```

Open `http://127.0.0.1:8080`.
Select a task.
Select a model set.
Select **Run benchmark**.

The dashboard uses demo responses when it sends `demo: true`.
Change this value to `false` in `web/app.js` when the model routes are ready.
Set `approved_for_live: true` only after approval.
Record the deployed artifact digest before a live run.

You can also open `web/index.html` directly.
The page will use standalone demo data when the Rust service is not available.

## Connect a model

Edit `config/models.yaml`.
Use an endpoint that supports `POST /v1/chat/completions`.
Set the exact model ID.
Set the exact quantisation.
Set the runtime and hardware.
Set the allowed data classes.
Set the token price when the route has a token price.
Replace the sample hosted gateway and prices before a real run.
Do not enter zero when a cost is unknown.
Use `null` until the cost basis is measured.

Do not put an API key in the YAML file.
Put the environment variable name in `api_key_env`.

## Add a benchmark

Edit `config/benchmarks.yaml`.
Give each task a stable ID.
Use synthetic data during the POC.
Define a deterministic grader when one is possible.
Define a human rubric for tasks that need expert judgement.

One task is not a benchmark pack.
Create at least 15 reviewed cases for each task class before a pilot decision.
Create more cases for a high-risk task.

## Project files

- `docs/POC_PLAN.md` gives the three-day plan and the benchmark method.
- `config/benchmarks.yaml` contains the first task pack.
- `config/models.yaml` contains the first model registry.
- `src` contains the Rust control plane.
- `web` contains the pitch dashboard.

## Decision rule

Use this order:

1. Check the data route.
2. Check the deterministic gate.
3. Check the expert score.
4. Check the latency target.
5. Select the lowest total cost among the eligible models.

Do not rank a failed model above a passed model because it is cheaper.
Do not promote a result with the `demo_only` state.

## Test the project

Run these commands:

```bash
cargo check --all-targets
cargo test --all-targets
node --check web/logic.js
node --check web/app.js
node tests/web_logic.test.js
node tests/html_contract.test.js
```

GitHub Actions runs the same checks on each pull request and each push to `main`.
