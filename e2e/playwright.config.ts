// The stories, played against the production stack. The tests share the bot
// and its clock, so they run one at a time.
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './tests',
  globalSetup: './global-setup.ts',
  workers: 1,
  fullyParallel: false,
  timeout: 300_000,
  expect: { timeout: 15_000 },
  reporter: [['list']],
  use: { trace: 'retain-on-failure' },
  projects: [
    { name: 'computer', use: { viewport: { width: 1280, height: 800 } } },
  ],
});
