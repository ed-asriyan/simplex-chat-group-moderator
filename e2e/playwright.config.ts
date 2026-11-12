// The stories, played against the production stack. The tests share the bot
// and its clock, so they run one at a time.
//
// The browser runs in Docker (`browser` in e2e/docker-compose.yml), so a page
// renders the same wherever the tests run, and a screenshot compares with the
// one approved under tests/__screenshots__/. Nothing is taken unless asked:
// a screenshot with nothing approved yet fails the test, and only the
// finalize stage takes one (AGENTS.md, *Workflow*).
import { defineConfig } from '@playwright/test';
import { BROWSER } from './harness/stack';

export default defineConfig({
  testDir: './tests',
  globalSetup: './global-setup.ts',
  workers: 1,
  fullyParallel: false,
  timeout: 300_000,
  reporter: [['list']],
  snapshotPathTemplate: '{testDir}/__screenshots__/{projectName}/{testFileName}/{arg}{ext}',
  updateSnapshots: 'none',
  expect: {
    timeout: 15_000,
    // A toast leaves after a few seconds, whenever the shot is taken.
    toHaveScreenshot: { animations: 'disabled', caret: 'hide', scale: 'css', stylePath: './harness/screenshot.css' },
  },
  use: {
    trace: 'retain-on-failure',
    // The browser goes out through the tests' machine: to its localhost, where
    // the editor is, and to what the editor loads (lz-string, its font).
    connectOptions: { wsEndpoint: BROWSER, exposeNetwork: '*' },
  },
  projects: [
    { name: 'computer', use: { viewport: { width: 1280, height: 800 } } },
    // The editor on a phone: the editor's own stories only, for the bot's are
    // the same on any screen.
    {
      name: 'phone',
      testMatch: /g-editor\.spec\.ts/,
      use: { viewport: { width: 390, height: 844 }, deviceScaleFactor: 3, isMobile: true, hasTouch: true },
    },
  ],
});
