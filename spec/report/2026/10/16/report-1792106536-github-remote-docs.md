## Report on task: Document GitHub remote, auth, and CI access

Ad-hoc work (owner added GitHub access; owner-directed doc update).

### Done

- Verified the setup (read-only): remote `origin` points at
  `git@github.com:lorien/rurex.git` (SSH); `gh` CLI authenticated as
  account `lorien` with the token in the OS keyring — no credentials
  in files or environment.
- dev.md gained a "Repository and CI" section: remote URL, auth
  mechanism (gh keyring; never in repo/docs/env), push command, CI
  trigger, run-watching commands.
- overview.md and README.md gained the repository URL; testing.md's
  CI paragraph now links the Actions page.
- No credential values appear anywhere in the documentation (work.md
  rule 2).

### Spec/ADR amendments

- Documentation-only change; no spec/ADR semantics touched. [acted]

### Future-task notes

- [open] First `git push -u origin main` starts the maiden CI run;
  confirm the 3.15 matrix entry builds (trim if PyO3 0.30.1 cannot
  target it — see the CI-workflow report).

### Tooling/process

- Token custody: OS keyring via `gh auth login`; the SSH remote
  rides on the user's SSH keys. Neither touches the repository.
