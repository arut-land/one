---
name: android
description: Select Jetpack Compose and Kotlin guidance for Arut's native Android surface, including Material design, state, effects, component APIs, motion, focus, testing, performance, and Gradle. Compose is the default UI stack.
---

# Android

Read the child skill for the current decision; add another only for a separate
concern. Manifest and resource XML remain valid, but use Compose for new UI.

| Task | Skill |
| --- | --- |
| Material design, layout, platform behavior | [Android design](android-design-guidelines/SKILL.md) |
| State ownership and effects | [State and effects](compose-state-and-effects/SKILL.md) |
| Modifiers, slots, reusable APIs | [Component design](compose-component-design/SKILL.md) |
| Motion API selection | [Animations](compose-animations/SKILL.md) |
| Keyboard and focus | [Focus navigation](compose-focus-navigation/SKILL.md) |
| Recomposition and runtime cost | [Performance](compose-performance/SKILL.md) |
| Semantics and input verification | [UI testing](compose-ui-testing-patterns/SKILL.md) |
| Coroutines and Flow | [Concurrency](kotlin-concurrency-and-flow/SKILL.md) |
| Kotlin types and API ownership | [API design](kotlin-api-design/SKILL.md) |
| Branching and exhaustiveness | [Control flow](kotlin-control-flow/SKILL.md) |
| Builds and failure diagnosis | [Gradle](gradle-run/SKILL.md) |
| Physical device benchmark comparison | [Benchmark comparison](android-benchmark-comparison/SKILL.md) |
| Actual Kotlin library publication | [Library release](release-kotlin-library/SKILL.md) |

The retained [detailed router](using-chrisbanes-skills/SKILL.md) can help when
Kotlin and Compose concerns overlap; skip it when the choice is already clear.
Use the repository's mise tasks and Gradle wrapper. Match examples to the
installed Kotlin/Compose versions, preserving shared Rust state ownership and
platform lifecycle behavior.
