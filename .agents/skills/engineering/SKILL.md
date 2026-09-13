---
name: engineering
description: Select guidance for state modeling, architecture, code simplification, or review across Arut surfaces. Planning, agent orchestration, and GitHub workflows apply only when explicitly requested.
---

# Engineering

Read only the child skill that answers the current task, then its relevant
references. Add another only for a distinct concern.

For state ownership, select `state-management`; for a cleanup, select `code-simplifier` or `deslop`; for architectural boundaries, select `architecture-patterns`. Do not impose backend layering on a small UI adapter.

`implement-with-subagents`, `run-github-project`, `shepherd`, and `to-plan` are explicit workflow references. Loading this group does not invoke them, authorize delegation, or authorize remote mutations. Their provider requirements apply only to a requested workflow.

- [architecture-patterns](architecture-patterns/SKILL.md)
- [code-review-excellence](code-review-excellence/SKILL.md)
- [code-simplifier](code-simplifier/SKILL.md)
- [deslop](deslop/SKILL.md)
- [state-management](state-management/SKILL.md)
- [implement-with-subagents](implement-with-subagents/SKILL.md)
- [run-github-project](run-github-project/SKILL.md)
- [shepherd](shepherd/SKILL.md)
- [to-plan](to-plan/SKILL.md)
