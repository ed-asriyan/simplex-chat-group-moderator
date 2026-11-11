// The production stack in Docker (`docker-compose.yml`, overridden by
// `e2e/docker-compose.yml`): brought up once per run by `global-setup.ts`,
// left running afterwards for a look at its logs, and taken down at the start
// of the next.
//
// The bot and its SimpleX CLI read the tests' clock, which only ever moves
// forward and never shows a time before the stack started: the SMP server's
// certificate is made then, and the messaging layer is not built for a clock
// that goes back. So a test never names a time: it moves the clock forward
// from where it reads (`advance`).
//
// Environment:
// - `E2E_SKIP_BUILD=1` uses the images already built instead of building them;
// - `E2E_COMPOSE_FILES` adds compose files of this machine's own, separated by
//   `:` (a machine whose kernel has no IPv6 needs one for the SMP server);
// - `E2E_DEBUG=1` prints every event the people's CLI hands the tests.
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const ROOT = path.resolve(__dirname, '..', '..');
export const PERSON_WS = 'ws://127.0.0.1:15225';
export const EDITOR = 'http://127.0.0.1:18080';
/** The browser of the tests, in Docker (`browser` in e2e/docker-compose.yml). */
export const BROWSER = 'ws://127.0.0.1:13000/';
// The browser's image is the Playwright the tests run, so the two always agree.
const PLAYWRIGHT = { E2E_PLAYWRIGHT_VERSION: require('@playwright/test/package.json').version as string };
const CLOCK_DIR = path.join(os.tmpdir(), 'simplex-chat-moderator-bot-e2e-clock');
const CLOCK_FILE = path.join(CLOCK_DIR, 'now');
const STATE_FILE = path.join(CLOCK_DIR, 'stack.json');

function files(): string[] {
  const extra = (process.env.E2E_COMPOSE_FILES || '').split(':').filter(Boolean);
  return ['docker-compose.yml', 'e2e/docker-compose.yml', ...extra].flatMap((f) => ['-f', f]);
}

export function compose(args: string[], env: Record<string, string> = {}): string {
  return execFileSync('docker', ['compose', ...files(), ...args], {
    cwd: ROOT,
    env: { ...process.env, ...PLAYWRIGHT, ...env },
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    maxBuffer: 64 * 1024 * 1024,
  });
}

function logs(service: string): string {
  return execFileSync('docker', ['compose', ...files(), 'logs', service], { cwd: ROOT, env: { ...process.env, ...PLAYWRIGHT }, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
}

function waitForLog(service: string, pattern: RegExp): string {
  const deadline = Date.now() + 120_000;
  for (;;) {
    const found = logs(service).match(pattern);
    if (found) return found[1];
    if (Date.now() > deadline) throw new Error(`${service} did not log ${pattern} in time`);
    execFileSync('sleep', ['1']);
  }
}

/** Builds and starts the stack; returns the bot's address. */
export function start(): string {
  fs.mkdirSync(CLOCK_DIR, { recursive: true });
  fs.writeFileSync(CLOCK_FILE, '+0');
  if (!process.env.E2E_SKIP_BUILD) compose(['build']);
  compose(['down', '-v', '--remove-orphans']);
  compose(['up', '-d', 'smp-server']);
  const smp = waitForLog('smp-server', /Server address: (smp:\/\/\S+)/);
  compose(['up', '-d', '--no-build'], { E2E_SMP_SERVER: smp, E2E_CLOCK_DIR: CLOCK_DIR });
  const address = waitForLog('bot', /Bot address: (\S+)/);
  waitForLog('browser', /(Listening on) /);
  fs.writeFileSync(STATE_FILE, JSON.stringify({ address, now: new Date().toISOString() }));
  return address;
}

function state(): { address: string; now: string } {
  return JSON.parse(fs.readFileSync(STATE_FILE, 'utf8'));
}

/** The bot's SimpleX address, as it printed it on start. */
export function botAddress(): string {
  return state().address;
}

/** What the bot's clock reads now. */
export function now(): Date {
  const offset = Number(fs.readFileSync(CLOCK_FILE, 'utf8').trim());
  return new Date(Date.now() + offset * 1000);
}

/**
 * Makes the bot's clock, and its CLI's, read `at` from here on, and keep
 * running. `at` is never before a time the clock has already shown.
 */
export function setNow(at: Date) {
  const last = new Date(state().now);
  if (at < last) throw new Error(`the clock moves forward only: ${at.toISOString()} is before ${last.toISOString()}`);
  const offset = Math.round((at.getTime() - Date.now()) / 1000);
  fs.writeFileSync(CLOCK_FILE, `${offset >= 0 ? '+' : ''}${offset}`);
  fs.writeFileSync(STATE_FILE, JSON.stringify({ ...state(), now: at.toISOString() }));
}

/** Moves the clock forward by `minutes` from what it reads now. */
export function advance(minutes: number) {
  setNow(new Date(now().getTime() + minutes * 60_000));
}
