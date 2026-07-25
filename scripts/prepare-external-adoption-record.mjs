import { createHash } from "node:crypto";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  loadExternalAdoptionBundle,
  validateExternalAdoptionBundle,
} from "./external-adoption-v1.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const privateRoot = resolve(
  repositoryRoot,
  "benchmarks",
  "external-adoption-v1",
  "private",
);
const outputRoot = resolve(repositoryRoot, "target", "external-adoption-intake");
const typeToLedger = Object.freeze({
  interview: "interviews",
  incident: "incidents",
  pilot: "pilots",
});

export class ExternalAdoptionPreparationError extends Error {
  constructor(message) {
    super(message);
    this.name = "ExternalAdoptionPreparationError";
  }
}

function fail(message) {
  throw new ExternalAdoptionPreparationError(message);
}

function within(base, candidate) {
  const child = relative(base, candidate);
  return child === "" || (!child.startsWith("..") && !isAbsolute(child));
}

function regularInput(value, role, maximumBytes) {
  if (typeof value !== "string" || value === "") fail(`${role} path is required`);
  const path = resolve(value);
  if (!existsSync(path)) fail(`${role} does not exist`);
  const metadata = lstatSync(path);
  if (metadata.isSymbolicLink() || !metadata.isFile()) {
    fail(`${role} must be a regular non-link file`);
  }
  if (metadata.size === 0 || metadata.size > maximumBytes) {
    fail(`${role} must contain 1..${maximumBytes} bytes`);
  }
  if (within(repositoryRoot, path) && !within(privateRoot, path)) {
    fail(`${role} inside the repository must be under the ignored private custody directory`);
  }
  return { path, metadata };
}

function safeOutput(value) {
  if (typeof value !== "string" || value === "") fail("output path is required");
  const path = resolve(value);
  if (!within(outputRoot, path) || path === outputRoot) {
    fail("output must be a file under target/external-adoption-intake");
  }
  if (existsSync(path)) fail("output already exists");
  return path;
}

function parseJson(bytes, role) {
  let value;
  try {
    value = JSON.parse(bytes.toString("utf8"));
  } catch {
    fail(`${role} must be one UTF-8 JSON object`);
  }
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(`${role} must be one JSON object`);
  }
  return value;
}

function rejectSensitivePublicRecord(record) {
  const text = JSON.stringify(record);
  const forbidden = [
    [/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/iu, "email address"],
    [/-----BEGIN [A-Z ]+PRIVATE KEY-----/u, "private key"],
    [/\bgh[pousr]_[A-Za-z0-9]{20,}\b/u, "GitHub token"],
    [/\bAKIA[0-9A-Z]{16}\b/u, "AWS access key"],
    [/\b[A-Za-z]:\\\\[^"]+/u, "absolute Windows path"],
  ];
  for (const [pattern, label] of forbidden) {
    if (pattern.test(text)) fail(`public record appears to contain a ${label}`);
  }
}

export function prepareExternalAdoptionRecord(
  {
    type,
    source: sourceValue,
    record: recordValue,
    output: outputValue,
  },
  { root = repositoryRoot } = {},
) {
  if (root !== repositoryRoot) {
    fail("preparation root must be the PliegoCSS repository");
  }
  const ledger = typeToLedger[type];
  if (!ledger) fail("type must be interview, incident, or pilot");
  const source = regularInput(sourceValue, "private source", 10 * 1024 * 1024);
  const recordInput = regularInput(recordValue, "redacted record", 256 * 1024);
  if (source.path === recordInput.path) fail("private source and redacted record must be different files");
  const output = safeOutput(outputValue);

  const sourceBytes = readFileSync(source.path);
  const sourceSha256 = `sha256:${createHash("sha256").update(sourceBytes).digest("hex")}`;
  const record = parseJson(readFileSync(recordInput.path), "redacted record");
  if (!record.source || typeof record.source !== "object" || Array.isArray(record.source)) {
    fail("redacted record.source must be an object");
  }
  record.source.sha256 = sourceSha256;
  if (
    type !== "pilot" &&
    (typeof record.source.custodyRef !== "string" ||
      !record.source.custodyRef.startsWith(`private:g7/${ledger}/`))
  ) {
    fail(`redacted record.source.custodyRef must start with private:g7/${ledger}/`);
  }
  rejectSensitivePublicRecord(record);

  const bundle = loadExternalAdoptionBundle(root);
  bundle[ledger].records.push(record);
  validateExternalAdoptionBundle(bundle, { root, verifyFiles: false });

  mkdirSync(dirname(output), { recursive: true });
  const bytes = `${JSON.stringify(record, null, 2)}\n`;
  writeFileSync(output, bytes, { encoding: "utf8", flag: "wx" });
  return {
    schemaVersion: 1,
    kind: "pliegocss-external-adoption-preparation",
    type,
    id: record.id,
    sourceBytes: source.metadata.size,
    sourceSha256,
    output,
    emittedBytes: Buffer.byteLength(bytes),
  };
}

export function parsePreparationArguments(args) {
  const values = {};
  for (const argument of args) {
    const match = /^--(type|source|record|output)=(.+)$/u.exec(argument);
    if (!match || match[1] in values) fail(`unknown or duplicate argument ${argument}`);
    values[match[1]] = match[2];
  }
  if (Object.keys(values).length !== 4) {
    fail(
      "usage: prepare-external-adoption-record --type=<interview|incident|pilot> --source=<private-file> --record=<redacted-json> --output=<target-path>",
    );
  }
  return values;
}

if (
  process.argv[1] &&
  resolve(process.argv[1]) === resolve(fileURLToPath(import.meta.url))
) {
  try {
    const result = prepareExternalAdoptionRecord(
      parsePreparationArguments(process.argv.slice(2)),
    );
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
  } catch (error) {
    process.stderr.write(`external adoption preparation: ${error.message}\n`);
    process.exitCode = 1;
  }
}
