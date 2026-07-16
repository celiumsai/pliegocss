# pliego-css-agent

`pliego-css-agent` owns PliegoCSS's closed repair-proposal, change-plan, apply, Change Receipt, and
post-change verification contracts. It accepts only exact UTF-8 byte edits linked to verified,
unexcepted, low-risk finding suggestions. Plans bind the finding document, source snapshots,
budgets, required checks, byte-edit patch, and their own canonical payload hash.

Apply requires an external exact-plan token and publishes sources plus a change-only receipt through
a lock/staging/rollback boundary. Verification uses only built-in check kinds from a closed policy;
schema 1.4 adds exact fixed-profile PliegoRS Chromium evidence to Rust test evidence, standard CSS,
token-graph integrity, and identity-bound in-process CSS budgets while retaining canonical
1.0/1.1/1.2/1.3 read support.
`run-tests` is a separate explicit boundary for the single locked/offline Cargo workspace profile;
`run-browser` is a separate explicit boundary for one pinned PliegoRS/CDP profile. `verify` never
launches either runner. Supplementary policies/evidence are exact path/bytes/SHA-256 inputs
below the source root. Plan or policy content cannot provide an executable or shell command.
Every check publishes its complete canonical FindingDocument beside the receipt with
create-if-absent, receipt-last rollback. See the
[repair-plan reference](https://github.com/celiums/pliegocss/blob/main/docs/reference/repair-plan-schema.md).
The post-change policy, supplementary-input, evidence, and receipt contract is documented in the
[repair-verification reference](https://github.com/celiums/pliegocss/blob/main/docs/reference/repair-verification-schema.md).
