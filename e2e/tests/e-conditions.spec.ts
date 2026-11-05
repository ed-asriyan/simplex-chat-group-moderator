// E. Conditions: what the owner sees when adding each in the editor, and what
// it matches, with its reason line.
import { expect, Locator, Page, test } from '@playwright/test';
import { Person, settingsLink } from '../harness/person';
import { advance } from '../harness/stack';
import { addCondition, anotherMember, apply, emptyEditor, moderated, moderatedAnyway, notice, rule, rules, scene, untouched } from './helpers';

const PIXEL = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
const lines = (n: number) => Array.from({ length: n }, (_, i) => `line ${i + 1}`).join('\n');
const words = (n: number) => Array.from({ length: n }, (_, i) => `w${i}`).join(' ');

/** A number field: its label, its default and its limits. */
async function number(pane: Locator, title: string, value: string, min?: string, max?: string) {
  const field = pane.getByRole('spinbutton', { name: title, exact: true });
  await expect(field).toHaveValue(value);
  if (min !== undefined) await expect(field).toHaveAttribute('min', min);
  if (max !== undefined) await expect(field).toHaveAttribute('max', max);
}

/** A list field: its label with no entries, and its limits, shown in bulk edit. */
async function list(pane: Locator, title: string, limits?: string) {
  const label = new RegExp(`^${title.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')} · 0`);
  await expect(pane.getByText(label)).toBeVisible();
  if (limits) {
    await pane.getByRole('button', { name: 'bulk edit' }).first().click();
    await expect(pane.getByText(limits)).toBeVisible();
  }
}

/** No field at all under the condition. */
async function noField(pane: Locator) {
  await expect(pane.getByRole('spinbutton')).toHaveCount(0);
  await expect(pane.getByRole('textbox')).toHaveCount(0);
  await expect(pane.getByRole('checkbox')).toHaveCount(0);
}

async function adding(page: Page, type: string): Promise<Locator> {
  await emptyEditor(page);
  return addCondition(page, type);
}

// ---- Text ----

// US-E1
test('message contains any of these words', async ({ page }) => {
  await list(await adding(page, 'Contains a listed word'), 'Words / Phrases', 'Up to 10000 entries, 100 characters each.');
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam', 'blow job', 'ass', 'badword'] })));
  await untouched(s, 'a classic move');
  // 'sраm' has a Cyrillic р and а.
  for (const text of ['spam', 'SPAM', 'sраm', '𝐬𝐩𝐚𝐦', 'ｓｐａｍ', 'ɯɐds', 's.p.a.m', 'spaaaam', '@spam', 'spams']) {
    await test.step(text, () => moderated(s, text, ["contains word: 'spam'"]));
  }
  await moderated(s, 'b@dw0rd', ["contains word: 'badword'"]);
  await moderated(s, 'blowjob', ["contains word: 'blow job'"]);
});

// US-E2
test('message exactly matches any of these texts', async ({ page }) => {
  const pane = await adding(page, 'Exactly matches a listed text');
  await list(pane, 'Texts', 'Up to 10000 entries, 1000 characters each.');
  await expect(pane.getByRole('checkbox', { name: 'Case sensitive' })).not.toBeChecked();
  const s = await scene(rules(rule({ type: 'MatchesExactMessage', messages: ['buy now'], case_sensitive: false })));
  await moderated(s, 'Buy Now ', ['buy now']);
  await untouched(s, 'buy now please');
  await apply(s, rules(rule({ type: 'MatchesExactMessage', messages: ['buy now'], case_sensitive: true })));
  await untouched(s, 'Buy Now');
  await moderated(s, 'buy now', ['buy now']);
});

// US-E3
test('message matches any of these regex patterns', async ({ page }) => {
  await list(await adding(page, 'Matches a regex pattern'), 'Regex Patterns', 'Up to 100 entries, 200 characters each.');
  const s = await scene(rules(rule({ type: 'MatchesRegex', patterns: ['\\d{4,}'] })));
  await moderated(s, 'call 5551234', ["matches regex pattern: '\\d{4,}'"]);
  await untouched(s, 'call 555');
});

// US-E4
test('message contains repeated sequence', async ({ page }) => {
  const pane = await adding(page, 'Contains a repeated sequence');
  await number(pane, 'Minimum Repeats in a Row', '5', '2');
  await number(pane, 'Minimum Sequence Length', '1', '1', '50');
  const s = await scene(rules(rule({ type: 'ContainsRepeatedSequence', min_repeats: 5, min_length: 1 })));
  await moderated(s, 'hahahahaha', ['"ha" repeated 5 times in a row']);
  await moderated(s, 'aaaaa', ['"a" repeated 5 times in a row']);
  await moderated(s, '!!!!!', ['"!" repeated 5 times in a row']);
  // Letter case aside; the story does not say which case the reason quotes.
  await moderatedAnyway(s, 'AAAAA');
  await untouched(s, 'haha');
});

// ---- Links ----

// US-E5
test('message contains a link to any of these websites', async ({ page }) => {
  await list(await adding(page, 'Link to a listed website'), 'Websites', 'Up to 10000 entries, 100 characters each.');
  const s = await scene(rules(rule({ type: 'ContainsLinksInList', domains: ['spam.example'] })));
  await moderated(s, 'see https://spam.example/offer', ['spam.example']);
  await moderatedAnyway(s, 'see www.spam.example');
  await moderated(s, 'see hxxp://spam.example', ['spam.example']);
  await untouched(s, 'see https://simplex.chat');
});

// US-E6
test('message contains a link to a website outside this list', async ({ page }) => {
  await list(await adding(page, 'Link outside this list'), 'Websites');
  const s = await scene(rules(rule({ type: 'ContainsLinksOutsideList', domains: ['simplex.chat'] })));
  await untouched(s, 'https://simplex.chat/blog');
  await moderated(s, 'https://spam.example', ['spam.example']);
  await apply(s, rules(rule({ type: 'ContainsLinksOutsideList', domains: [] })));
  await moderated(s, 'https://simplex.chat/blog', ['simplex.chat']);
});

// US-E7
test('message contains a link outside top 100 websites', async ({ page }) => {
  await list(await adding(page, 'Link outside top 100'), 'Extra Websites Added to Top 100');
  const s = await scene(rules(rule({ type: 'ContainsLinksOutsideTop100', domains: [] })));
  await untouched(s, 'https://github.com/simplex-chat');
  await moderated(s, 'https://spam.example', ['spam.example']);
});

// ---- Shape and size ----

// US-E8
test('message is empty or blank', async ({ page }) => {
  await noField(await adding(page, 'Is Empty or Blank'));
  const s = await scene(rules(rule({ type: 'IsBlank' })));
  await moderated(s, '   ', ['empty message'], null);
  await moderated(s, '​', ['empty message'], '​');
  await untouched(s, { type: 'image', text: '', image: PIXEL });
});

// US-E9
test('message contains invisible characters', async ({ page }) => {
  await noField(await adding(page, 'Contains Invisible Characters'));
  const s = await scene(rules(rule({ type: 'ContainsInvisibleCharacters' })));
  await moderated(s, 'hi​there', ['invisible characters']);
  await untouched(s, 'family 👨‍👩‍👧');
});

// US-E10
test('message exceeds max characters', async ({ page }) => {
  await number(await adding(page, 'Exceeds max characters'), 'Max Characters', '1000', '1');
  const s = await scene(rules(rule({ type: 'ExceedsMaxCharacters', max_characters: 1000 })));
  await moderated(s, 'x'.repeat(1203), ['1203 characters'], `${'x'.repeat(300)}...`);
  await untouched(s, 'x'.repeat(1000));
});

// US-E11
test('message exceeds max words', async ({ page }) => {
  await number(await adding(page, 'Exceeds max words'), 'Max Words', '150', '1');
  const s = await scene(rules(rule({ type: 'ExceedsMaxWords', max_words: 150 })));
  await moderated(s, words(152), ['152 words'], `${words(152).slice(0, 300)}...`);
  await untouched(s, words(150));
});

// US-E12
test('message exceeds max lines', async ({ page }) => {
  const pane = await adding(page, 'Exceeds max lines');
  await number(pane, 'Max Lines', '10', '1');
  await number(pane, 'Characters per Line (Screen Wrap)', '40', '0');
  const s = await scene(rules(rule({ type: 'ExceedsMaxLines', max_lines: 10, chars_per_line: 40 })));
  await moderated(s, lines(12), ['12 lines']);
  await untouched(s, lines(10));
  // A line longer than 40 characters wraps: 9 short lines and one of 81 are 12.
  await moderated(s, `${lines(9)}\n${'y'.repeat(81)}`, ['12 lines']);
  await apply(s, rules(rule({ type: 'ExceedsMaxLines', max_lines: 10, chars_per_line: 0 })));
  await untouched(s, `${lines(9)}\n${'y'.repeat(81)}`);
});

// ---- Attachments ----

// US-E13
test('message contains an image', async ({ page }) => {
  await noField(await adding(page, 'Contains an image'));
  const s = await scene(rules(rule({ type: 'ContainsImage' })));
  await moderated(s, { type: 'image', text: '', image: PIXEL }, ['contains an image'], null);
  await untouched(s, 'just text');
  // A caption is the message's text for every other condition.
  await apply(s, rules(rule({ type: 'ContainsWords', keywords: ['spam'] })));
  await moderated(s, { type: 'image', text: 'spam', image: PIXEL }, ["contains word: 'spam'"]);
});

// US-E14
test('message contains a video', async ({ page }) => {
  await noField(await adding(page, 'Contains a video'));
  const s = await scene(rules(rule({ type: 'ContainsVideo' })));
  await moderated(s, { type: 'video', text: '', image: PIXEL, duration: 1 }, ['contains a video'], null);
  await untouched(s, { type: 'image', text: '', image: PIXEL });
});

// US-E15
test('message contains a voice message', async ({ page }) => {
  await noField(await adding(page, 'Contains a voice message'));
  const s = await scene(rules(rule({ type: 'ContainsVoiceMessage' })));
  await moderated(s, { type: 'voice', text: '', duration: 1 }, ['contains a voice message'], null);
  await untouched(s, 'just text');
});

// US-E16
test('message contains a file', async ({ page }) => {
  await noField(await adding(page, 'Contains a file'));
  const s = await scene(rules(rule({ type: 'ContainsFile' })));
  await moderated(s, { type: 'file', text: '' }, ['contains a file'], null);
  await untouched(s, { type: 'image', text: '', image: PIXEL });
});

// ---- The author ----

// US-E17
test('author hits message rate limit', async ({ page }) => {
  const pane = await adding(page, 'Message rate limit');
  await number(pane, 'Number of Messages', '5', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  const s = await scene(rules(rule({ type: 'AuthorHitsMessageRateLimit', message_count: 5, time_window_minutes: 1 })));
  for (let i = 1; i <= 4; i++) await untouched(s, `message ${i}`);
  await moderated(s, 'message 5', ['author sent 5 messages in 1 min']);
});

// US-E18
test('author hits character rate limit', async ({ page }) => {
  const pane = await adding(page, 'Character rate limit');
  await number(pane, 'Number of Characters', '2000', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  const s = await scene(rules(rule({ type: 'AuthorHitsCharacterRateLimit', character_count: 2000, time_window_minutes: 1 })));
  await untouched(s, 'a'.repeat(1070));
  await moderated(s, 'b'.repeat(1070), ['author sent 2140 characters in 1 min'], `${'b'.repeat(300)}...`);
});

// US-E19
test('author hits line rate limit', async ({ page }) => {
  const pane = await adding(page, 'Line rate limit');
  await number(pane, 'Number of Lines', '30', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  await number(pane, 'Characters per Line (Screen Wrap)', '40', '0');
  const s = await scene(rules(rule({ type: 'AuthorHitsLineRateLimit', line_count: 30, time_window_minutes: 1, chars_per_line: 40 })));
  await untouched(s, lines(20));
  await moderated(s, lines(11), ['author sent 31 lines in 1 min']);
});

// US-E20
test('author hits moderation rate limit', async ({ page }) => {
  const pane = await adding(page, 'Moderation rate limit');
  await number(pane, 'Number of Moderated Messages', '3', '1');
  await number(pane, 'Time Window (Minutes)', '60', '1', '60');
  const s = await scene(rules(
    rule({ type: 'AuthorHitsModerationRateLimit', message_count: 3, time_window_minutes: 60 }),
    rule({ type: 'ContainsWords', keywords: ['spam'] }),
  ));
  await moderated(s, 'spam 1', ["contains word: 'spam'"]);
  await moderated(s, 'spam 2', ["contains word: 'spam'"]);
  await moderated(s, 'spam 3', ['author had 3 messages moderated in 60 min', "contains word: 'spam'"]);
  // Then the author's next messages too, while the window holds them.
  await moderated(s, 'hello', ['author had 3 messages moderated in 60 min']);
});

// US-E21
test('author joined recently', async ({ page }) => {
  await number(await adding(page, 'Joined recently'), 'Joined Less Than (Minutes) Ago', '60', '1');
  // A member who was in the group before the bot joined never matches.
  const alice = await Person.join('Alice');
  await alice.message();
  const group = await alice.createGroup('Crypto Chat');
  const early = await Person.create('Erin');
  const earlyGroup = await early.joinGroup(await alice.groupLink(group));
  await alice.inviteBot(group, 'owner');
  await alice.message();
  const link = await alice.message();
  const id = link.match(/bot_id=(\d+)/)![1];
  expect(await alice.ask(settingsLink(id, rules(rule({ type: 'AuthorJoinedRecently', time_window_minutes: 60 }))))).toBe('✅ Group settings updated.');
  await early.post(earlyGroup, 'I was here first');
  await early.seesNothing(4000);
  // A member who joins after it does, for an hour.
  const bob = await Person.create('Bob');
  const bobGroup = await bob.joinGroup(await alice.groupLink(group));
  const item = await bob.post(bobGroup, 'hi');
  expect(await bob.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await alice.message()).toMatch(/^🛡 I moderated the message in \*\*Crypto Chat\*\*\.\n\n\*\*The message:\*\*\n> hi\n\n\*\*Reason:\*\*\n- author joined \d+ min ago \(less than 60 min\)\n/);
  advance(61);
  await bob.post(bobGroup, 'an hour later');
  await bob.seesNothing(4000);
});

// ---- The group ----

/** Bob and Carol alternate, `n` messages in all, the last one moderated with `reason`. */
async function together(n: number, text: (i: number) => string, reason: string) {
  const s = await scene();
  const carol = await anotherMember(s);
  return { s, carol, n, text, reason };
}

// US-E22
test('group hits message rate limit', async ({ page }) => {
  const pane = await adding(page, 'Group message rate limit');
  await number(pane, 'Number of Messages', '30', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  const { s, carol } = await together(30, String, '');
  await apply(s, rules(rule({ type: 'GroupHitsMessageRateLimit', message_count: 30, time_window_minutes: 1 })));
  for (let i = 1; i < 30; i++) {
    if (i % 2) await s.member.post(s.memberGroup, `m${i}`);
    else await carol.person.post(carol.group, `m${i}`);
  }
  await s.member.seesNothing(3000);
  await moderated(s, 'm30', ['group received 30 messages in 1 min']);
});

// US-E23
test('group hits character rate limit', async ({ page }) => {
  const pane = await adding(page, 'Group character rate limit');
  await number(pane, 'Number of Characters', '10000', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  const { s, carol } = await together(2, String, '');
  await apply(s, rules(rule({ type: 'GroupHitsCharacterRateLimit', character_count: 10000, time_window_minutes: 1 })));
  await carol.person.post(carol.group, 'c'.repeat(5200));
  await carol.person.seesNothing(3000);
  await moderated(s, 'b'.repeat(5200), ['group received 10400 characters in 1 min'], `${'b'.repeat(300)}...`);
});

// US-E24
test('group hits line rate limit', async ({ page }) => {
  const pane = await adding(page, 'Group line rate limit');
  await number(pane, 'Number of Lines', '150', '1');
  await number(pane, 'Time Window (Minutes)', '1', '1', '60');
  await number(pane, 'Characters per Line (Screen Wrap)', '40', '0');
  const { s, carol } = await together(2, String, '');
  await apply(s, rules(rule({ type: 'GroupHitsLineRateLimit', line_count: 150, time_window_minutes: 1, chars_per_line: 40 })));
  await carol.person.post(carol.group, lines(76));
  await carol.person.seesNothing(3000);
  await moderated(s, lines(76), ['group received 152 lines in 1 min']);
});

// ---- AI ----
// What these conditions match is decided by OpenAI and OpenRouter, which the
// end-to-end tests do not reach (AGENTS.md, Workflow); what the owner sees
// when adding them is the editor's.

// US-E25
test('flagged by OpenAI Omni: its fields', async ({ page }) => {
  const pane = await adding(page, 'Flagged by OpenAI Omni');
  await expect(pane.getByRole('textbox', { name: /^OpenAI API Key/ })).toHaveAttribute('type', 'password');
  for (const category of ['Sexual', 'Sexual (minors)', 'Harassment', 'Harassment (threatening)', 'Hate', 'Hate (threatening)', 'Illicit', 'Illicit (violent)', 'Self-harm', 'Self-harm (intent)', 'Self-harm (instructions)', 'Violence', 'Violence (graphic)']) {
    const field = pane.getByRole('combobox', { name: category, exact: true });
    await expect(field).toHaveValue(/./);
    await expect(field.locator('option:checked')).toHaveText('OpenAI decides');
    await expect(field.locator('option')).toHaveText(['Off', 'OpenAI decides', 'Min score, %']);
  }
  await number(pane, 'Attempts', '3', '1', '5');
  await number(pane, 'Pause Between Attempts (Seconds)', '1', '0', '10');
});

// US-E26
test('flagged by OpenRouter Instruction: its fields', async ({ page }) => {
  const pane = await adding(page, 'Flagged by OpenRouter Instruction');
  await expect(pane.getByRole('textbox', { name: /^OpenRouter API Key/ })).toHaveAttribute('type', 'password');
  const model = pane.getByRole('combobox', { name: 'Model', exact: true });
  await expect(model.locator('option:checked')).toHaveText('google/gemini-2.5-flash-lite');
  await expect(model.locator('option')).toHaveText([
    'google/gemini-2.5-flash-lite', 'mistralai/mistral-small-3.2-24b-instruct', 'meta-llama/llama-3.3-70b-instruct',
    'meta-llama/llama-4-maverick', 'openai/gpt-4.1-nano', 'openai/gpt-4o-mini', 'openai/gpt-4.1-mini',
    'anthropic/claude-haiku-4.5', 'openai/gpt-4o', 'openai/gpt-4.1',
  ]);
  await expect(pane.getByRole('textbox', { name: /^Instruction/ })).toHaveAttribute('maxlength', '4000');
  await number(pane, 'Earlier Messages Sent as Context', '0', '0', '10');
  await number(pane, 'Attempts', '3', '1', '5');
  await number(pane, 'Pause Between Attempts (Seconds)', '1', '0', '10');
});

// ---- Combining conditions ----

// US-E27
test('all of these conditions match', async ({ page }) => {
  const pane = await adding(page, 'All of these (AND)');
  await expect(pane.getByRole('button', { name: '+ Add to AND' })).toBeVisible();
  const s = await scene(rules(rule({ type: 'All', conditions: [
    { type: 'ContainsWords', keywords: ['spam'] }, { type: 'ContainsLinksInList', domains: ['casino.example'] },
  ] })));
  await untouched(s, 'spam');
  await untouched(s, 'https://casino.example');
  await moderatedAnyway(s, 'spam at https://casino.example');
});

// US-E28
test('any of these conditions match', async ({ page }) => {
  const pane = await adding(page, 'Any of these (OR)');
  await expect(pane.getByRole('button', { name: '+ Add to OR' })).toBeVisible();
  const s = await scene(rules(rule({ type: 'Any', conditions: [
    { type: 'ContainsWords', keywords: ['spam'] }, { type: 'ContainsLinksInList', domains: ['spam.example'] },
  ] })));
  await moderatedAnyway(s, 'spam');
  await moderatedAnyway(s, 'https://spam.example');
  await untouched(s, 'hello');
});

// US-E29
test('this condition does not match', async ({ page }) => {
  const pane = await adding(page, 'Does not match');
  await expect(pane.getByRole('combobox', { name: 'Condition type' })).toHaveCount(2);
  const s = await scene(rules(rule({ type: 'Not', condition: { type: 'ExceedsMaxCharacters', max_characters: 1000 } })));
  await moderated(s, 'hi', ['does not match: has more than 1000 characters']);
  await untouched(s, 'x'.repeat(1001));
});

// US-E30
test('combinations nest up to 8 levels and 64 conditions', async () => {
  // `n` combinations nested, then a word: n + 1 levels. AND and OR alternate,
  // each beside a word of its own, so none folds into its parent.
  const deep = (n: number): unknown =>
    n === 0 ? { type: 'ContainsWords', keywords: ['w0'] } : { type: n % 2 ? 'All' : 'Any', conditions: [{ type: 'ContainsWords', keywords: [`w${n}`] }, deep(n - 1)] };
  const s = await scene();
  expect(await s.owner.ask(settingsLink(s.id, rules(rule(deep(7)))))).toBe('✅ Group settings updated.');
  expect(await s.owner.ask(settingsLink(s.id, rules(rule(deep(8)))))).toMatch(/^❌ \*\*Failed to update group settings:\*\*\n\n> \S/);
  const wide = { type: 'Any', conditions: Array.from({ length: 64 }, (_, i) => ({ type: 'ContainsWords', keywords: [`w${i}`] })) };
  expect(await s.owner.ask(settingsLink(s.id, rules(rule(wide))))).toMatch(/^❌ \*\*Failed to update group settings:\*\*\n\n> \S/);
  const fits = { type: 'Any', conditions: Array.from({ length: 63 }, (_, i) => ({ type: 'ContainsWords', keywords: [`w${i}`] })) };
  expect(await s.owner.ask(settingsLink(s.id, rules(rule(fits))))).toBe('✅ Group settings updated.');
});

void notice;
