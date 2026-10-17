# rurex — Python bindings to the Rust regex crate

Read `spec/docs/index.md` (the catalog and reading order of all design
documents) before working in this repository, then the workflow files in
`spec/skills/` — `work.md`, `task_tracking.md`, `report_tracking.md`,
`adr_tracking.md`. Those are the authoritative starting points: pick the
highest-priority `new` task from `spec/docs/plan.md`, present your plan,
and keep the docs in sync with the code.

## Credentials

- The GitHub token for `gh` lives in `.env` (`GITHUB_TOKEN=…`,
  gitignored). It is not exported automatically; without it `gh`
  silently falls back to a weaker keyring token that cannot write
  (release creation fails with HTTP 403). Before any `gh` command:
  `set -a; . ./.env; set +a`. Never print or commit the value.
