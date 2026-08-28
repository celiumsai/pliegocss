import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import {
  ExternalAdoptionPreparationError,
  prepareExternalAdoptionRecord,
} from "./prepare-external-adoption-record.mjs";

const root = resolve(import.meta.dirname, "..");
const privateBase = resolve(
  root,
  "benchmarks",
  "external-adoption-v1",
  "private",
);
const outputBase = resolve(root, "target", "external-adoption-intake");
mkdirSync(privateBase, { recursive: true });
mkdirSync(outputBase, { recursive: true });
const privateDirectory = mkdtempSync(join(privateBase, "preparation-test-"));
const outputDirectory = mkdtempSync(join(outputBase, "preparation-test-"));

try {
  const sourcePath = join(privateDirectory, "source.bin");
  const recordPath = join(privateDirectory, "record.json");
  const outputPath = join(outputDirectory, "interview.json");
  const privateBytes = Buffer.from(
    "PRIVATE INTERVIEW MATERIAL: participant@example.invalid\n",
  );
  writeFileSync(sourcePath, privateBytes);
  writeFileSync(
    recordPath,
    `${JSON.stringify(
      {
        id: "interview-preparation-test",
        participantId: "participant-preparation-test",
        organizationId: "organization-preparation-test",
        relationship: "external-community",
        conductedAt: "2026-07-20T00:00:00Z",
        consent: {
          status: "recorded",
          recordedAt: "2026-07-19T00:00:00Z",
          researchUse: true,
          publicRedactedRecord: true,
        },
        redaction: { status: "reviewed", containsPersonalData: false },
        source: {
          custodyRef:
            "private:g7/interviews/interview-preparation-test",
          sha256: "sha256:pending",
        },
        review: {
          reviewer: "reviewer-preparation-test",
          reviewedAt: "2026-07-21T00:00:00Z",
          status: "approved",
        },
        findings: ["A reviewed redacted finding."],
        relatedIncidentIds: [],
        relatedPilotIds: [],
      },
      null,
      2,
    )}\n`,
  );
  const result = prepareExternalAdoptionRecord({
    type: "interview",
    source: sourcePath,
    record: recordPath,
    output: outputPath,
  });
  const expectedDigest = `sha256:${createHash("sha256").update(privateBytes).digest("hex")}`;
  assert.equal(result.sourceSha256, expectedDigest);
  const emitted = readFileSync(outputPath, "utf8");
  assert.equal(JSON.parse(emitted).source.sha256, expectedDigest);
  assert(!emitted.includes("PRIVATE INTERVIEW MATERIAL"));
  assert(!emitted.includes("participant@example.invalid"));
  assert(!emitted.includes(sourcePath));

  assert.throws(
    () =>
      prepareExternalAdoptionRecord({
        type: "interview",
        source: resolve(root, "benchmarks", "external-adoption-v1", "authority.json"),
        record: recordPath,
        output: join(outputDirectory, "unsafe-source.json"),
      }),
    (error) =>
      error instanceof ExternalAdoptionPreparationError &&
      /ignored private custody directory/u.test(error.message),
  );

  const sensitiveRecordPath = join(privateDirectory, "sensitive-record.json");
  const sensitive = JSON.parse(readFileSync(recordPath, "utf8"));
  sensitive.findings = ["Contact participant@example.invalid for details."];
  writeFileSync(sensitiveRecordPath, `${JSON.stringify(sensitive, null, 2)}\n`);
  assert.throws(
    () =>
      prepareExternalAdoptionRecord({
        type: "interview",
        source: sourcePath,
        record: sensitiveRecordPath,
        output: join(outputDirectory, "sensitive.json"),
      }),
    (error) =>
      error instanceof ExternalAdoptionPreparationError &&
      /email address/u.test(error.message),
  );
} finally {
  rmSync(privateDirectory, { recursive: true, force: true });
  rmSync(outputDirectory, { recursive: true, force: true });
}

process.stdout.write("external adoption record preparation: pass\n");
