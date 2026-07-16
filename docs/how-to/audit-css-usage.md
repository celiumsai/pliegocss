# Audit generated CSS usage without unsafe deletion

Use the usage report to separate what the application graph can reach, what a declared test/browser
snapshot actually exercised, and what PliegoCSS still cannot know.

## 1. Generate the complete inventory

Start report-only, without pruning:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --usage-report \
  --asset-plan \
  --control
```

Review `pliego.usage.json`:

- `reachable + unavailable + unknown + retain` means the graph can reach the style but no runtime
  observation was supplied;
- `unreachable + unavailable + dead + candidate` is a complete structural candidate;
- `unknown + unavailable + unknown + blocked` means application evidence is missing; and
- a StyleId with mixed reachable/unreachable origins remains reachable and retains every origin.

## 2. Add scoped positive observations

Collect hits from the same immutable build. Generate a closed sidecar using the universe and
reachability hashes from the first report, declare every exercised context and known dynamic gap,
then rerun with:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --usage-report \
  --observations pliego.observations.json \
  --asset-plan \
  --control
```

Interpret absence conservatively. A reachable entry not hit by this snapshot becomes `unobserved`
and remains `retain`. Add routes/states or investigate dynamic construction; do not label it dead.
Context arrays are producer labels recorded for review, not topology claims independently verified
by PliegoCSS.

## 3. Review structural candidates

For each `dead/candidate` entry, inspect:

- every exact source origin and owner component;
- the reachability input digest and adapter version;
- route/island roots in the report;
- dynamic inputs and contexts the observation producer could not cover; and
- whether the same StyleId appears in another bundle-qualified entry.

Fix stale application metadata before pruning. A missing origin or owner is an error, not a dead
candidate.

## 4. Prune only generated whole StyleIds

After review, add the explicit flag:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable \
  --usage-report \
  --asset-plan \
  --project-index \
  --control
```

The report keeps the all-compiled tombstone with `selected: false` and
`removalDisposition: removed`. Rebuild without `--prune-unreachable` to roll back. No authored Rust,
ordinary CSS, individual declaration, or theme variable is edited.

## 5. Retain an exceptional dead StyleId by explicit policy

Use retention only when a reviewed external consumer genuinely needs a StyleId that the complete
application graph cannot reach—for example, a mail renderer outside route/island topology. Do not
use it to hide stale component ownership.

Create `pliego.retention.json` inside the bundle-plan directory. Copy `universeSha256` from the
current report and the exact reachability digest from `application.inputSha256`, then name the exact
bundle-qualified entry:

```json
{
  "schemaVersion": 1,
  "universeSha256": "64-lowercase-hex-digits",
  "reachabilitySha256": "64-lowercase-hex-digits",
  "entries": [
    {
      "id": "external-email-renderer",
      "bundleId": "email",
      "styleId": "32-lowercase-hex-digits",
      "justification": "Consumed by the external mail renderer outside application routing."
    }
  ]
}
```

Prefer the canonical builder in `pliego-css-usage` for producer tooling. The sidecar must be
non-empty and cannot use wildcards, selectors, class names, paths, dates, or unqualified StyleIds.

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --prune-unreachable \
  --usage-report \
  --retention pliego.retention.json \
  --asset-plan \
  --project-index \
  --control
```

The exact retained entry remains `unreachable + dead`, but becomes `selected: true` with
`retention.state: retained` and `removalDisposition: policy-retained`. Other dead entries stay
removed. The report, Asset Plan, and Project Index use schema 2 with
`reachable-or-retained-style-ids`; route/island membership is not invented for the retained style.
A positive observation contradicting its unreachable proof still aborts the build.

## 6. Gate drift read-only

Repeat the identical command with `--check`. It compares every expected output byte, including the
usage report and control receipt, and never repairs a mismatch:

```console
pliego-cssc bundle ... --usage-report --asset-plan --control --check
pliego-cssc bundle ... --usage-report --prune-unreachable \
  --retention pliego.retention.json --asset-plan --control --check
```

Commit the reachability, observation, and retention inputs alongside the reviewed report, or store
them in the same content-addressed evidence system. Re-review the retention policy whenever either
bound input changes. Hashes detect drift; they do not authenticate the producer.
