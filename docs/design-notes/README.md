# Design Notes

Design rationale, trade-offs, and decision records for aifed.

| Note                                        | Topics                                                   |
| ------------------------------------------- | -------------------------------------------------------- |
| [Philosophy](philosophy.md)                 | AI-First principles, One Way, Help vs Skill              |
| [Locator](locator.md)                       | Hashline, Hash algorithm                                 |
| [Edit Model](edit-model.md)                 | Unified edit, Operators, Replacement, Virtual line       |
| [Operator Evolution](operator-evolution.md) | History of `=` (remove → restore)                        |
| [Filepath & Locator](filepath-locator.md)   | Filepath separation, Symbol locator, Workspace detection |
| [Batch & Atomicity](batch-atomicity.md)     | Batch operations, Hash mismatch, Failure handling        |
| [Error Handling](error-handling.md)         | Error format, Exit codes                                 |
| [Architecture](architecture.md)             | CLI + Daemon, Configuration, History vs Git              |
| [Open Questions](open-questions.md)         | Future considerations                                    |

For command usage, see the [CLI Reference](../reference/README.md).
