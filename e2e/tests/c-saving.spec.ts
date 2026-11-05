// C. Saving a group's configuration.
import { expect, test } from '@playwright/test';
import { Person, settingsLink } from '../harness/person';
import { EDITOR } from '../harness/stack';
import { apply, moderated, rule, rules, scene, untouched } from './helpers';

const failed = (reason: string) => `❌ **Failed to update group settings:**\n\n> ${reason}`;
const SPAM = rules(rule({ type: 'ContainsWords', keywords: ['spam'] }));

// US-C1
test('the link from the editor replaces the whole configuration', async () => {
  const s = await scene();
  await untouched(s, 'spam');
  // Anywhere in a message.
  expect(await s.owner.ask(`here it is: ${settingsLink(s.id, SPAM)} thanks`)).toBe('✅ Group settings updated.');
  await moderated(s, 'spam', ["contains word: 'spam'"]);
  await apply(s, rules(rule({ type: 'ContainsWords', keywords: ['scam'] })));
  await untouched(s, 'spam');
  await moderated(s, 'scam', ["contains word: 'scam'"]);
});

// US-C2
test('a configuration that is not valid saves nothing', async () => {
  const s = await scene(SPAM);
  const cases: [unknown, string][] = [
    [rules({ actions: [], condition: { type: 'ContainsWords', keywords: ['x'] } }), 'A rule has no actions. Add at least one action to it, or remove the rule.'],
    [rules(rule({ type: 'Not', condition: { type: 'AuthorHitsModerationRateLimit', message_count: 3, time_window_minutes: 60 } })),
      "'Author Hits Moderation Rate Limit' cannot be placed under a 'Not' condition, because it already depends on what the other rules do with this message."],
  ];
  for (const [config, reason] of cases) {
    expect(await s.owner.ask(settingsLink(s.id, config))).toBe(failed(reason));
  }
  for (const config of [
    rules(rule({ type: 'All', conditions: [] })),
    rules(rule({ type: 'MatchesRegex', patterns: ['(unclosed'] })),
    rules(rule({ type: 'ExceedsMaxCharacters', max_characters: 0 })),
  ]) {
    expect(await s.owner.ask(settingsLink(s.id, config))).toMatch(/^❌ \*\*Failed to update group settings:\*\*\n\n> \S/);
  }
  const garbage = `${EDITOR}/#bot_id=${s.id}&config=garbage`;
  expect(await s.owner.ask(garbage)).toBe(failed('Could not decompress the settings from the URL. Open the editor link again, make your changes, and send back the updated URL.'));
  // The old configuration stays.
  await moderated(s, 'spam', ["contains word: 'spam'"]);
});

// US-C3
test('a link for a group of someone else is refused', async () => {
  const s = await scene(SPAM);
  const mallory = await Person.join('Mallory');
  await mallory.message();
  expect(await mallory.ask(settingsLink(s.id, rules()))).toMatch(
    new RegExp(`^❌ \\*\\*Failed to update group settings:\\*\\*\\n\\n> User \\d+ is not the owner of group ${s.id}$`),
  );
  await moderated(s, 'spam', ["contains word: 'spam'"]);
});

// US-C4: an AI condition's key is checked with OpenAI or OpenRouter, which the
// end-to-end tests do not reach (AGENTS.md, Workflow).

// US-C5
test('a link to the editor without its settings is no settings', async () => {
  const s = await scene();
  for (const text of [`${EDITOR}/`, `${EDITOR}/#config=abc`, `${EDITOR}/#bot_id=${s.id}`]) {
    expect(await s.owner.ask(text), text).toBe('Unknown command. Send /help to see what I can do.');
  }
});
