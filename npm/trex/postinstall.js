"use strict";

const { mkdir, writeFile, chmod } = require("node:fs/promises");
const { existsSync } = require("node:fs");
const { dirname, join } = require("node:path");
const https = require("node:https");

const PACKAGE_VERSION = require("./package.json").version;

const TARGET_TRIPLES = {
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
  "win32-arm64": "aarch64-pc-windows-msvc",
};

function platformBinaryName() {
  return process.platform === "win32" ? "trex.exe" : "trex";
}

function resolveTargetTriple() {
  return TARGET_TRIPLES[`${process.platform}-${process.arch}`];
}

function resolveDownloadUrl() {
  const triple = resolveTargetTriple();
  if (!triple) {
    return null;
  }

  const extension = process.platform === "win32" ? ".exe" : "";
  const filename = `trex-${triple}${extension}`;
  const baseUrl =
    process.env.TREX_RELEASE_BASE_URL ||
    `https://github.com/dreamyoungs/trex/releases/download/v${PACKAGE_VERSION}`;

  return `${baseUrl}/${filename}`;
}

function download(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    const req = https.get(url, (res) => {
      if (res.statusCode && res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        if (redirects >= 5) {
          reject(new Error("too many redirects"));
          return;
        }
        resolve(download(res.headers.location, redirects + 1));
        return;
      }

      if (res.statusCode !== 200) {
        reject(new Error(`HTTP ${res.statusCode}`));
        return;
      }

      const chunks = [];
      res.on("data", (chunk) => chunks.push(chunk));
      res.on("end", () => resolve(Buffer.concat(chunks)));
    });

    req.on("error", reject);
  });
}

async function main() {
  if (process.env.TREX_SKIP_DOWNLOAD === "1") {
    console.log("[trex] skip binary download (TREX_SKIP_DOWNLOAD=1)");
    return;
  }

  const targetUrl = resolveDownloadUrl();
  if (!targetUrl) {
    console.warn(
      `[trex] no prebuilt binary for ${process.platform}-${process.arch}. ` +
        "Install TREX manually and set TREX_BIN."
    );
    return;
  }

  const destination = join(__dirname, "bin", platformBinaryName());
  if (existsSync(destination)) {
    return;
  }

  try {
    const binary = await download(targetUrl);
    await mkdir(dirname(destination), { recursive: true });
    await writeFile(destination, binary);
    if (process.platform !== "win32") {
      await chmod(destination, 0o755);
    }
    console.log(`[trex] downloaded prebuilt binary: ${targetUrl}`);
  } catch (error) {
    console.warn(
      `[trex] binary download failed (${String(error)}). ` +
        "Falling back to system binary. Set TREX_BIN if needed."
    );
  }
}

main().catch((error) => {
  console.warn(`[trex] postinstall warning: ${String(error)}`);
});
