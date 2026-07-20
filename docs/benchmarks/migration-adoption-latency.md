# Migration adoption latency

Date: 2026-07-19

Status: **passing local Windows release-binary measurement**

The representative Vite/Tailwind project was measured with the optimized `pliego-cssc` binary after two warm-up runs and twenty recorded process invocations per command.

| Command | Median | p95 | Maximum |
|---|---:|---:|---:|
| `migration-project-inventory .` | 9.513 ms | 10.135 ms | 15.019 ms |
| `migration-project-plan .` | 9.843 ms | 10.532 ms | 10.576 ms |

The executable gate is `scripts/measure-migration-adoption.mjs`. It fails when either p95 exceeds 250 ms. These figures include Windows process startup, bounded discovery, double reads, canonical serialization, and plan hashing. They do not measure large public projects, network filesystems, antivirus variance, or hosted CI.

The next adoption gap is therefore not local latency. It is migration semantics: current replacement only adds a preparation marker and deliberately does not convert Tailwind utilities or preserve visual equivalence through a real codemod.
