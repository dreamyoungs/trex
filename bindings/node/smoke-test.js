"use strict";

const assert = require("node:assert/strict");
const { readFileSync } = require("node:fs");
const { resolve } = require("node:path");

const trex = require("./index.js");

const pdfPath = resolve(__dirname, "../../tests/pdfs/twotables.pdf");
const pdfBuffer = readFileSync(pdfPath);

const tables = trex.extract(pdfPath, { mode: "Auto" });
assert.equal(tables.length, 2, "extract() should return 2 tables for twotables.pdf");

const tablesFromBuffer = trex.extractFromBuffer(pdfBuffer, { mode: "Auto" });
assert.equal(
  tablesFromBuffer.length,
  2,
  "extractFromBuffer() should return 2 tables for twotables.pdf"
);

const csv = trex.extractCsv(pdfPath, { mode: "Auto" });
assert.ok(csv.includes("\n"), "extractCsv() should return non-empty CSV output");

const csvFromBuffer = trex.extractCsvFromBuffer(pdfBuffer, { mode: "Auto" });
assert.ok(
  csvFromBuffer.includes("\n"),
  "extractCsvFromBuffer() should return non-empty CSV output"
);

console.log("TREX Node smoke test passed.");
