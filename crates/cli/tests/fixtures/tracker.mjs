#!/usr/bin/env node
// Offline fake tracker for the black-box tests (mirrors examples/tracker.mjs).
import { readFileSync, writeFileSync } from 'node:fs';

const [dataFile, cmd, ...rest] = process.argv.slice(2);
if (!dataFile || !cmd) {
  console.error('usage: tracker.mjs <datafile> <fetch|transition> [KEY STATUS]');
  process.exit(2);
}

const raw = readFileSync(dataFile, 'utf8');

if (cmd === 'fetch') {
  process.stdout.write(raw);
} else if (cmd === 'transition') {
  const [key, status] = rest;
  if (!key || !status) {
    console.error('transition requires KEY and STATUS');
    process.exit(2);
  }
  const data = JSON.parse(raw);
  const items = Array.isArray(data) ? data : data.issues;
  let hit = false;
  for (const it of items) {
    if (it.key === key) {
      if (it.fields) it.fields.status.name = status;
      else it.state = status;
      hit = true;
    }
  }
  if (!hit) {
    console.error(`no issue with key ${key}`);
    process.exit(1);
  }
  writeFileSync(dataFile, JSON.stringify(data, null, 2));
  console.log(`transitioned ${key} -> ${status}`);
} else {
  console.error(`unknown command: ${cmd}`);
  process.exit(2);
}
