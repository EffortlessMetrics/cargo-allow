# Gemini CLI Context for cargo-allow

@./AGENTS.md
@./CLAUDE.md
@./docs/campaigns/cargo-allow-0.2.0.md

## Agent Operating Profile

- Single execution entrypoint is campaign controller #3768 (`docs/campaigns/cargo-allow-0.2.0.md`).
- After context changes, run `/memory reload`.
- To inspect active workspace skills, run `/skills reload` and `/skills list`.
- Follow the two distinct workspace skills:
  - [`.agents/skills/cargo-allow-0.2-campaign/SKILL.md`](.agents/skills/cargo-allow-0.2-campaign/SKILL.md): Reversible issue-first implementation and orchestration for the active #3768 campaign.
  - [`.agents/skills/review-current-head/SKILL.md`](.agents/skills/review-current-head/SKILL.md): Independent exact base/head PR review and merge-readiness verification.
- Do not treat task narration, task completion, or a green latest workflow run as repository authority.
- Do not move, delete, or recreate `v0.2.0-rc.1` or perform another `rc.1` upload.
- Stop before root decisions, candidate freeze (#2501), external release authorization (#3760), and final publication execution (#2502).
