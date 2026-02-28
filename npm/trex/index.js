"use strict";

const { spawn } = require("node:child_process");
const { mkdtemp, rm, writeFile } = require("node:fs/promises");
const { existsSync } = require("node:fs");
const { randomUUID } = require("node:crypto");
const { join, resolve } = require("node:path");
const { tmpdir } = require("node:os");

const VALID_MODES = new Set(["auto", "lattice", "stream", "dl"]);
const VALID_DL_FALLBACK = new Set(["auto", "lattice", "stream"]);

function resolveBinaryPath(options) {
  if (options.binPath) {
    return options.binPath;
  }

  if (process.env.TREX_BIN) {
    return process.env.TREX_BIN;
  }

  const bundled = resolveBundledBinaryPath();
  if (bundled) {
    return bundled;
  }

  return "trex";
}

function resolveBundledBinaryPath() {
  const binaryName = process.platform === "win32" ? "trex.exe" : "trex";
  const localPath = resolve(__dirname, "bin", binaryName);
  if (existsSync(localPath)) {
    return localPath;
  }
  return null;
}

function normalizePages(pages) {
  if (pages == null) {
    return undefined;
  }

  if (typeof pages === "string") {
    return pages;
  }

  if (Array.isArray(pages)) {
    if (pages.length === 0) {
      return undefined;
    }

    return pages.join(",");
  }

  throw new TypeError("`pages` must be a string or number[]");
}

function buildExtractArgs(pdfPath, options, format) {
  if (typeof pdfPath !== "string" || pdfPath.length === 0) {
    throw new TypeError("`pdfPath` must be a non-empty string");
  }

  const args = ["extract", pdfPath, "--format", format];

  if (options.mode != null) {
    if (!VALID_MODES.has(options.mode)) {
      throw new TypeError("`mode` must be one of: auto, lattice, stream, dl");
    }
    args.push("--mode", options.mode);
  }

  const pages = normalizePages(options.pages);
  if (pages) {
    args.push("--pages", pages);
  }

  if (options.dlModel) {
    args.push("--dl-model", options.dlModel);
  }
  if (options.dlMinConfidence != null) {
    args.push("--dl-min-confidence", String(options.dlMinConfidence));
  }
  if (options.dlFallback != null) {
    if (!VALID_DL_FALLBACK.has(options.dlFallback)) {
      throw new TypeError("`dlFallback` must be one of: auto, lattice, stream");
    }
    args.push("--dl-fallback", options.dlFallback);
  }

  if (options.eventLog) {
    args.push("--event-log", options.eventLog);
  }
  if (options.eventDocumentKey) {
    args.push("--event-document-key", options.eventDocumentKey);
  }
  if (options.eventTenantId) {
    args.push("--event-tenant-id", options.eventTenantId);
  }
  if (options.eventRequestId) {
    args.push("--event-request-id", options.eventRequestId);
  }
  if (options.eventFeedbackTag) {
    args.push("--event-feedback-tag", options.eventFeedbackTag);
  }
  if (options.eventTrainingOptIn) {
    args.push("--event-training-opt-in");
  }

  return args;
}

function runTrex(args, options) {
  const timeoutMs = options.timeoutMs ?? 120000;
  const binaryPath = resolveBinaryPath(options);

  return new Promise((resolve, reject) => {
    const child = spawn(binaryPath, args, {
      stdio: ["ignore", "pipe", "pipe"],
      env: options.env || process.env,
    });

    let stdout = "";
    let stderr = "";
    let finished = false;

    const timeout = setTimeout(() => {
      if (finished) {
        return;
      }
      finished = true;
      child.kill("SIGKILL");
      reject(new Error(`TREX command timed out after ${timeoutMs}ms`));
    }, timeoutMs);

    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");

    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });

    child.on("error", (error) => {
      if (finished) {
        return;
      }
      finished = true;
      clearTimeout(timeout);
      reject(
        new Error(
          `Failed to spawn TREX binary (${binaryPath}). ` +
            "Set options.binPath or TREX_BIN, or reinstall package to trigger postinstall binary download. " +
            error.message
        )
      );
    });

    child.on("close", (code) => {
      if (finished) {
        return;
      }
      finished = true;
      clearTimeout(timeout);

      if (code !== 0) {
        const message = stderr.trim() || `TREX exited with code ${code}`;
        reject(new Error(message));
        return;
      }

      resolve({ stdout, stderr, code });
    });
  });
}

async function extract(pdfPath, options = {}) {
  const args = buildExtractArgs(pdfPath, options, "json");
  const result = await runTrex(args, options);

  try {
    return JSON.parse(result.stdout || "[]");
  } catch (error) {
    throw new Error(`Failed to parse TREX JSON output: ${error.message}`);
  }
}

async function extractCsv(pdfPath, options = {}) {
  const args = buildExtractArgs(pdfPath, options, "csv");
  const result = await runTrex(args, options);
  return result.stdout;
}

async function withTempPdf(buffer, callback) {
  if (!Buffer.isBuffer(buffer)) {
    throw new TypeError("`buffer` must be a Node.js Buffer");
  }

  const tempDir = await mkdtemp(join(tmpdir(), "trex-node-"));
  const tempFile = join(tempDir, `${randomUUID()}.pdf`);

  try {
    await writeFile(tempFile, buffer);
    return await callback(tempFile);
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
}

async function extractFromBuffer(buffer, options = {}) {
  return withTempPdf(buffer, (tempFile) => extract(tempFile, options));
}

async function extractCsvFromBuffer(buffer, options = {}) {
  return withTempPdf(buffer, (tempFile) => extractCsv(tempFile, options));
}

module.exports = {
  extract,
  extractCsv,
  extractFromBuffer,
  extractCsvFromBuffer,
};
