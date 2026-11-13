// G. The web editor, as the owner uses it in a browser: the group's overview,
// a rule's page and the page for a new rule. Every element is reached by its
// role and its accessible name.
import { expect, Page, test } from '@playwright/test';
import { configOf } from '../harness/person';
import { EDITOR } from '../harness/stack';
import {
  JOINS, MESSAGES, applyDialog, currentLink, editorUrl, emptyEditor, joinRule, looks, named, newRule, open, openRule, pickable,
  rule, rules, savedConfig, scene, section, toGroup,
} from './helpers';

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());
const words = (n: number) => Array.from({ length: n }, (_, i) => `word${i}`);
const msg = (keywords: string[]) => ({ type: 'ContainsWords', keywords });
const MODERATE = [{ type: 'ModerateMessage' }];
const KICK = [{ type: 'KickAuthor', delete_all_messages: false }];
const BANNED = named('Banned words', 'message_posted', msg(['spam', 'casino']), MODERATE);
const NEWCOMERS = named('Links from newcomers', 'message_posted', {
  type: 'All',
  conditions: [{ type: 'AuthorJoinedRecently', time_window_minutes: 60 }, { type: 'ContainsLinksOutsideTop100', domains: [] }],
}, [{ type: 'SetAuthorObserver', duration_minutes: 60 }, { type: 'ModerateMessage' }]);
const FLOOD = named('Flood', 'message_posted', { type: 'AuthorHitsMessageRateLimit', message_count: 5, time_window_minutes: 1 }, KICK);
const SPAM_NAMES = named('Spam names', 'member_joined', { type: 'MemberNameContainsWords', keywords: ['casino'] }, KICK);
const UNNAMED = rule({ type: 'AuthorHitsMessageRateLimit', message_count: 5, time_window_minutes: 1 }, KICK);

/** What a person reads in `locator`, its whitespace as the browser shows it. */
async function textOf(locator: ReturnType<Page['locator']>): Promise<string> {
  return (await locator.innerText()).replace(/\s+/g, ' ');
}

const mode = (page: Page, name: 'Notifications' | 'Silent' | 'Dry') => page.getByRole('radio', { name: new RegExp(`^${name} `) });
const nameField = (page: Page) => page.getByRole('textbox', { name: 'Name', exact: true });
const row = (page: Page, name: string) => page.getByRole('listitem').filter({ has: page.getByRole('button', { name, exact: true }) });
const undo = (page: Page) => page.getByRole('button', { name: 'Undo', exact: true });
const redo = (page: Page) => page.getByRole('button', { name: 'Redo', exact: true });
const menuOf = (page: Page, name: string) => page.getByRole('button', { name: `More for “${name}”`, exact: true });
const choose = (page: Page, item: string) => page.getByRole('menuitem', { name: item, exact: true });

/** The titles of the rules of a section, in the order they are checked. */
async function order(page: Page, name: string): Promise<string[]> {
  const grips = section(page, name).getByRole('button', { name: /^Move “/ });
  return grips.evaluateAll((els) => els.map((e) => e.getAttribute('aria-label')!.replace(/^Move “|”$/g, '')));
}

async function rename(page: Page, text: string) {
  await nameField(page).fill(text);
  await nameField(page).press('Tab');
}

// ---------------------------------------------------------------------------
// The overview
// ---------------------------------------------------------------------------

// US-G1
test('the editor opens on the group overview: the group, its mode and its rules in two sections', async ({ page }) => {
  // At a fixed bot id first, for the screenshots; then by the group's own link.
  await open(page, editorUrl(rules(BANNED, NEWCOMERS, UNNAMED, { ...FLOOD, name: 'Flood', enabled: false }, SPAM_NAMES)));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Crypto Chat');
  await expect(page.getByText('Moderation settings · Moderator Bot')).toBeVisible();
  await expect(page.getByRole('region', { name: '🛡 Mode' })).toBeVisible();
  await expect(section(page, MESSAGES)).toContainText('Checked top to bottom.');
  await expect(section(page, JOINS)).toContainText('Checked top to bottom.');
  // A rule is one line: its name, or what it does; its actions in short under a name; its switch.
  await expect(row(page, 'Banned words')).toContainText('🗑 moderate');
  await expect(row(page, 'Links from newcomers')).toContainText('👁 observer · 🗑 moderate');
  await expect(row(page, 'IF author sent at least 5 messages in 1 min THEN 🚪 kick')).toBeVisible();
  await expect(row(page, 'Flood')).toContainText('Off');
  await expect(row(page, 'Spam names').getByRole('switch', { name: 'Spam names' })).toBeChecked();
  await looks(page, 'overview-rules');
  // By the group's own link: the bot's link carries the group's name.
  const s = await scene(rules(BANNED, SPAM_NAMES));
  await open(page, await currentLink(s));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Crypto Chat');
  await expect(row(page, 'Banned words')).toBeVisible();
});

// US-G1
test('an empty section says what that means', async ({ page }) => {
  await emptyEditor(page);
  await expect(section(page, MESSAGES)).toContainText('No rules yet. Nothing is moderated until you add one.');
  await expect(section(page, JOINS)).toContainText('No rules yet. Nobody is checked when they join.');
  await looks(page, 'overview-empty');
});

// US-G1
test('a link without a group name shows the group by its number', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED), '7', null));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Group 7');
});

// US-G1
test('the overview menu holds Edit all with AI, Import JSON and Export JSON', async ({ page }) => {
  await emptyEditor(page);
  await page.getByRole('button', { name: 'More', exact: true }).click();
  await expect(page.getByRole('menuitem')).toHaveText(['Edit all with AI', 'Import JSON', 'Export JSON']);
  await looks(page, 'overview-menu');
});

// US-G1
test('a rule of a type the editor does not know is kept as it is', async ({ page }) => {
  const unknown = named('From the future', 'message_posted', { type: 'FutureType', level: 3 }, MODERATE);
  await open(page, editorUrl(rules(unknown, BANNED)));
  await openRule(page, 'From the future');
  await expect(page.getByText('FutureType (unknown)', { exact: true })).toBeVisible();
  await expect(page.getByText('❓')).toBeVisible();
  await expect(page.getByRole('spinbutton')).toHaveCount(0);
  await page.getByRole('button', { name: 'About “FutureType (unknown)”', exact: true }).click();
  await expect(page.getByText("This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.")).toBeVisible();
  await looks(page, 'unknown-type');
  await toGroup(page);
  await mode(page, 'Dry').check();
  expect((await savedConfig(page)).rules[0]).toEqual(unknown);
});

// US-G1
test('a link whose settings cannot be read opens the editor empty, with a note', async ({ page }) => {
  await open(page, `${EDITOR}/#bot_id=7&config=garbage`);
  await expect(page.getByText('The settings in this link could not be read. Starting from scratch.')).toBeVisible();
  await expect(section(page, MESSAGES)).toContainText('No rules yet.');
  await looks(page, 'unreadable-settings');
});

// ---------------------------------------------------------------------------
// The mode
// ---------------------------------------------------------------------------

// US-G2
test('the mode is picked on the overview, each of three with its line', async ({ page }) => {
  await emptyEditor(page);
  const group = page.getByRole('radiogroup', { name: '🛡 Mode' });
  await expect(group.getByRole('radio')).toHaveCount(3);
  await expect(mode(page, 'Notifications')).toHaveAccessibleName(/Act and tell me/);
  await expect(mode(page, 'Silent')).toHaveAccessibleName(/Act without telling me/);
  await expect(mode(page, 'Dry')).toHaveAccessibleName(/Only tell me what it would do/);
  await expect(mode(page, 'Notifications')).toBeChecked();
  await mode(page, 'Silent').check();
  await looks(page, 'mode-silent');
  expect((await savedConfig(page)).mode).toBe('silent');
  await mode(page, 'Dry').check();
  expect((await savedConfig(page)).mode).toBe('dry');
});

// ---------------------------------------------------------------------------
// A rule's page
// ---------------------------------------------------------------------------

// US-G3
test('a rule opens on its own page, with its way back', async ({ page }) => {
  await open(page, editorUrl(rules(NEWCOMERS, UNNAMED)));
  await openRule(page, 'Links from newcomers');
  await expect(page.getByRole('button', { name: 'Back to rules' })).toBeVisible();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Links from newcomers');
  await expect(nameField(page)).toHaveValue('Links from newcomers');
  await expect(page.getByRole('radio', { name: '💬 Someone sends a message' })).toBeChecked();
  await expect(page.getByRole('radio', { name: '🚪 Someone joins' })).not.toBeChecked();
  for (const part of ['When', 'If', 'Then']) await expect(page.getByRole('heading', { name: part, exact: true })).toBeVisible();
  await expect(page.getByText('The bot runs them in this order.')).toBeVisible();
  // The conditions as rows titled by their type, the actions in the order the bot runs them.
  await expect(page.getByText('All of These Conditions Match (AND)', { exact: true })).toBeVisible();
  await expect(page.getByText('Author Joined Recently', { exact: true })).toBeVisible();
  await expect(page.getByText('Message Contains a Link Outside Top 100 Websites', { exact: true })).toBeVisible();
  await expect(page.getByText('Set the member as observer', { exact: true })).toBeVisible();
  await expect(page.getByText('Moderate the message', { exact: true })).toBeVisible();
  await looks(page, 'rule-page');
  // The way back, and the browser's.
  await toGroup(page);
  await expect(section(page, MESSAGES)).toBeVisible();
  await page.goBack();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Links from newcomers');
  await page.goBack();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Crypto Chat');
  // A rule with no name shows what it does as the placeholder of its name.
  await openRule(page, 'IF author sent at least 5 messages in 1 min THEN 🚪 kick');
  await expect(nameField(page)).toHaveValue('');
  await expect(nameField(page)).toHaveAttribute('placeholder', 'IF author sent at least 5 messages in 1 min THEN 🚪 kick');
});

// US-G3
test('a field shows its default and limits, and what it is for in a sentence, with More for the rest', async ({ page }) => {
  await open(page, editorUrl(rules(NEWCOMERS)));
  await openRule(page, 'Links from newcomers');
  const minutes = page.getByRole('spinbutton', { name: 'Joined Less Than (Minutes) Ago', exact: true });
  await expect(minutes).toHaveValue('60');
  await expect(minutes).toHaveAttribute('min', '1');
  const first = 'Matches if the author joined the group less than this many minutes before sending the message.';
  const rest = 'For example, 60 matches members during their first hour in the group.';
  await expect(page.getByText(first)).toBeVisible();
  await expect(page.getByText(rest)).toHaveCount(0);
  await page.getByRole('button', { name: 'More', exact: true }).first().click();
  await expect(page.getByText(rest)).toBeVisible();
  await looks(page, 'field-more');
  await page.getByRole('button', { name: 'Less', exact: true }).click();
  await expect(page.getByText(rest)).toHaveCount(0);
  // The type's full description is behind its ⓘ.
  await page.getByRole('button', { name: 'About “Author Joined Recently”', exact: true }).click();
  await expect(page.getByText('Matches if the author joined the group less than this many minutes before sending the message', { exact: false }).first()).toBeVisible();
  // A folded condition shows its settings in one line.
  await open(page, editorUrl(rules(BANNED)));
  await openRule(page, 'Banned words');
  await page.getByRole('button', { name: 'Collapse “Message Contains Any of These Words”', exact: true }).click();
  await expect(page.getByText('spam, casino', { exact: true })).toBeVisible();
});

// US-G3
test("a condition's menu holds its changes, and an action's holds Change type and Delete", async ({ page }) => {
  await open(page, editorUrl(rules(NEWCOMERS)));
  await openRule(page, 'Links from newcomers');
  await menuOf(page, 'All of These Conditions Match (AND)').click();
  await expect(page.getByRole('menuitem')).toHaveText(['Change type…', 'Wrap in NOT', 'Wrap in AND', 'Unwrap', 'Delete']);
  await looks(page, 'condition-menu');
  await page.keyboard.press('Escape');
  await menuOf(page, 'Set the member as observer').click();
  await expect(page.getByRole('menuitem')).toHaveText(['Change type…', 'Delete']);
});

// US-G3
test('Add condition lists the types the trigger takes, in tabs and with a search', async ({ page }) => {
  await open(page, editorUrl(rules(FLOOD, SPAM_NAMES)));
  await openRule(page, 'Flood');
  await page.getByRole('button', { name: 'Add condition', exact: true }).click();
  const list = page.getByRole('dialog', { name: 'Pick a condition' });
  const tabs = list.getByRole('tab');
  await expect(tabs).toHaveText(['All', 'Message text', 'Links', 'Shape and size', 'Attachments', 'Member', 'Group', 'Combine']);
  await expect(list.getByRole('tab', { name: 'All', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(list.getByRole('button', { name: /^Message Contains Any of These Words\s/ })).toBeVisible();
  await looks(page, 'condition-picker');
  await list.getByRole('tab', { name: 'Links', exact: true }).click();
  await expect(list.getByRole('button', { name: /^Message Contains a Link/ })).toHaveCount(3);
  await expect(list.getByRole('button', { name: /^Message Contains Any of These Words\s/ })).toHaveCount(0);
  await looks(page, 'condition-picker-links');
  await list.getByRole('tab', { name: 'All', exact: true }).click();
  await list.getByRole('searchbox', { name: 'Search the types' }).fill('top 100');
  await expect(list.getByRole('button', { name: /^Message Contains a Link Outside Top 100 Websites\s/ })).toBeVisible();
  await expect(list.getByRole('button', { name: /^Message Contains Any of These Words\s/ })).toHaveCount(0);
  // A bold word in a description stays in its line.
  await expect(list.getByRole('button', { name: /Google GitHub|Google, GitHub/ })).toBeVisible();
  await list.getByRole('button', { name: 'Close', exact: true }).click();
  // For when someone joins, only what reads a person's name and the combinations.
  await toGroup(page);
  await openRule(page, 'Spam names');
  await page.getByRole('button', { name: 'Add condition', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Pick a condition' }).getByRole('tab')).toHaveText(['All', 'Member', 'Combine']);
});

// US-G3
test('a rule for when someone joins can have no condition', async ({ page }) => {
  await open(page, editorUrl(rules(joinRule(undefined, [{ type: 'SetAuthorObserver', duration_minutes: 60 }], 'Quarantine newcomers'))));
  await expect(row(page, 'Quarantine newcomers')).toBeVisible();
  await openRule(page, 'Quarantine newcomers');
  await expect(page.getByText('Everyone who joins', { exact: true })).toBeVisible();
  await looks(page, 'join-rule-no-condition');
});

// US-G3
test('When moves the rule to the other section, and what does not work there is marked', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS, SPAM_NAMES)));
  await openRule(page, 'Banned words');
  await page.getByRole('radio', { name: '🚪 Someone joins' }).check();
  await expect(page.getByText("Doesn't work with this trigger", { exact: true })).toHaveCount(2);
  await looks(page, 'when-changed');
  await toGroup(page);
  // The end of the other section.
  expect(await order(page, MESSAGES)).toEqual(['Links from newcomers']);
  expect(await order(page, JOINS)).toEqual(['Spam names', 'Banned words']);
  // It stays marked until the condition and the action are changed or removed; and the bot would refuse it.
  await openRule(page, 'Banned words');
  await page.getByRole('radio', { name: '💬 Someone sends a message' }).check();
  await expect(page.getByText("Doesn't work with this trigger", { exact: true })).toHaveCount(0);
});

// US-G3
test('a rule is duplicated, and deleted', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS)));
  await openRule(page, 'Banned words');
  await page.getByRole('button', { name: 'More for this rule', exact: true }).click();
  await choose(page, 'Duplicate').click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Banned words (copy)');
  await toGroup(page);
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Banned words (copy)', 'Links from newcomers']);
  await openRule(page, 'Banned words (copy)');
  await page.getByRole('button', { name: 'More for this rule', exact: true }).click();
  await choose(page, 'Delete').click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Crypto Chat');
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Links from newcomers']);
});

// US-G3
test('a list is edited entry by entry, filtered, or as text', async ({ page }) => {
  await open(page, editorUrl(rules(named('Empty', 'message_posted', msg([]), MODERATE), named('Long', 'message_posted', msg(words(150)), MODERATE))));
  await openRule(page, 'Empty');
  const add = page.getByRole('textbox', { name: 'Add to Words / Phrases', exact: true });
  await add.fill('spam');
  await add.press('Enter');
  await expect(page.getByRole('button', { name: 'Remove spam' })).toBeVisible();
  await looks(page, 'list-entries');
  await add.fill('spam');
  await add.press('Enter');
  await expect(page.getByText('That entry is already in the list.')).toBeVisible();
  await page.getByRole('button', { name: 'Remove spam' }).click();
  await expect(page.getByRole('button', { name: 'Remove spam' })).toHaveCount(0);
  await toGroup(page);
  await openRule(page, 'Long');
  const filter = page.getByRole('searchbox', { name: 'Filter Words / Phrases' });
  await expect(filter).toHaveAttribute('placeholder', 'filter 150 entries');
  await filter.fill('word149');
  await expect(page.getByRole('button', { name: /^Remove word/ })).toHaveCount(1);
  await looks(page, 'long-list-filtered');
  await page.getByRole('button', { name: 'Bulk edit Words / Phrases' }).click();
  await expect(page.getByText('One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale. Up to 10000 entries, 100 characters each.')).toBeVisible();
  await looks(page, 'bulk-edit');
  await page.getByRole('button', { name: 'Show Words / Phrases as chips' }).click();
  await expect(add).toBeVisible();
});

// US-G4
test('changing a type keeps what both types have, and a composite unwraps', async ({ page }) => {
  const nested = named('Nested', 'message_posted', {
    type: 'Any',
    conditions: [{ type: 'All', conditions: [msg(['x']), { type: 'IsBlank' }] }, msg(['y'])],
  }, MODERATE);
  await open(page, editorUrl(rules(
    named('Both', 'message_posted', { type: 'All', conditions: [msg(['a']), msg(['b'])] }, MODERATE),
    named('Links', 'message_posted', { type: 'ContainsLinksInList', domains: ['spam.example'] }, MODERATE),
    nested,
  )));
  // All becomes Any, with its whole subtree.
  await openRule(page, 'Both');
  await menuOf(page, 'All of These Conditions Match (AND)').click();
  await choose(page, 'Change type…').click();
  await expect(page.getByRole('dialog', { name: "Change the condition's type" })).toBeVisible();
  await pickable(page, 'Any of These Conditions Match (OR)').click();
  await expect(page.getByText('Any of These Conditions Match (OR)', { exact: true })).toBeVisible();
  await expect(page.getByText('Message Contains Any of These Words', { exact: true })).toHaveCount(2);
  await toGroup(page);
  // A link condition becomes another with its domains.
  await openRule(page, 'Links');
  await menuOf(page, 'Message Contains a Link to Any of These Websites').click();
  await choose(page, 'Change type…').click();
  await pickable(page, 'Message Contains a Link to a Website Outside This List').click();
  await expect(page.getByRole('button', { name: 'Remove spam.example' })).toBeVisible();
  await toGroup(page);
  // A composite inside another is unwrapped: its conditions move up into the parent.
  await openRule(page, 'Nested');
  await menuOf(page, 'All of These Conditions Match (AND)').click();
  await looks(page, 'condition-menu-unwrap');
  await choose(page, 'Unwrap').click();
  await expect(page.getByText('All of These Conditions Match (AND)', { exact: true })).toHaveCount(0);
  await expect(page.getByText('Message Is Empty or Blank', { exact: true })).toBeVisible();
  await looks(page, 'unwrapped');
  const saved = await savedConfig(page);
  expect(saved.rules[0].condition.type).toBe('Any');
  expect(saved.rules[0].condition.conditions).toHaveLength(2);
  expect(saved.rules[1].condition).toEqual({ type: 'ContainsLinksOutsideList', domains: ['spam.example'] });
  expect(saved.rules[2].condition.conditions).toHaveLength(3);
});

// US-G4
test('an action changes its type too', async ({ page }) => {
  await open(page, editorUrl(rules(named('Kick', 'message_posted', msg(['a']), [{ type: 'ModerateMessage' }]))));
  await openRule(page, 'Kick');
  await menuOf(page, 'Moderate the message').click();
  await choose(page, 'Change type…').click();
  await expect(page.getByRole('dialog', { name: "Change the action's type" })).toBeVisible();
  await pickable(page, 'Kick the member').click();
  await expect(page.getByText('Kick the member', { exact: true })).toBeVisible();
  expect((await savedConfig(page)).rules[0].actions[0].type).toBe('KickAuthor');
});

// US-G5
test('the editor warns past the limits, and marks a covered action', async ({ page }) => {
  const tree = (nots: number, leaves: number): unknown =>
    nots === 0 ? { type: 'All', conditions: Array.from({ length: leaves }, (_, i) => msg([`w${i}`])) } : { type: 'Not', condition: tree(nots - 1, leaves) };
  await open(page, editorUrl(rules(
    named('Too deep', 'message_posted', tree(7, 62), MODERATE),
    named('Covered', 'message_posted', { type: 'IsBlank' }, [{ type: 'ModerateMessage' }, { type: 'KickAuthor', delete_all_messages: true }]),
  )));
  await openRule(page, 'Too deep');
  await expect(page.getByText('This rule has 70 conditions nested 9 levels deep. The bot accepts at most 64 conditions and 8 levels, so simplify it before applying — otherwise the rules will be rejected.')).toBeVisible();
  await looks(page, 'limits-warning');
  await toGroup(page);
  await openRule(page, 'Covered');
  await expect(page.getByText('covered', { exact: true })).toBeVisible();
  await expect(page.getByText('A stronger action on this rule already covers it, so the bot skips this one.')).toBeVisible();
  await looks(page, 'covered-action');
});

// ---------------------------------------------------------------------------
// Undo and redo
// ---------------------------------------------------------------------------

// US-G6
test('on the overview every change can be undone and redone', async ({ page }) => {
  await open(page, editorUrl(rules(FLOOD)));
  await expect(undo(page)).toBeHidden();
  await expect(redo(page)).toBeHidden();
  await mode(page, 'Dry').check();
  await undo(page).click();
  await expect(mode(page, 'Notifications')).toBeChecked();
  await expect(redo(page)).toBeVisible();
  await expect(undo(page)).toBeHidden();
  await looks(page, 'redo-shown');
  await redo(page).click();
  await expect(mode(page, 'Dry')).toBeChecked();
  // The keys, where there is a keyboard.
  if (!test.info().project.use.isMobile) {
    await page.locator('body').click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('Control+z');
    await expect(mode(page, 'Notifications')).toBeChecked();
    await page.keyboard.press('Control+Shift+z');
    await expect(mode(page, 'Dry')).toBeChecked();
    await page.keyboard.press('Control+z');
    await page.keyboard.press('Control+y');
    await expect(mode(page, 'Dry')).toBeChecked();
  }
  // Up to 20 steps: 25 changes, and the first five are not undone.
  const flood = page.getByRole('switch', { name: 'Flood', exact: true });
  for (let i = 0; i < 25; i++) await flood.click();
  for (let i = 0; i < 20; i++) await undo(page).click();
  await expect(undo(page)).toBeHidden();
  await expect(flood).not.toBeChecked();
});

// US-G6
test("on a rule's page Undo goes through what was changed there, and no rule that is not on the screen", async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS)));
  await openRule(page, 'Banned words');
  await expect(undo(page)).toBeHidden();
  await rename(page, 'Words');
  await expect(undo(page)).toBeVisible();
  await toGroup(page);
  // Another rule: nothing was changed on its page yet.
  await openRule(page, 'Links from newcomers');
  await expect(undo(page)).toBeHidden();
  await rename(page, 'Newcomers');
  await undo(page).click();
  await expect(nameField(page)).toHaveValue('Links from newcomers');
  await expect(undo(page)).toBeHidden();
  await expect(redo(page)).toBeVisible();
  if (!test.info().project.use.isMobile) {
    await page.locator('body').click({ position: { x: 5, y: 5 } });
    await page.keyboard.press('Control+z');
  }
  await toGroup(page);
  // The first rule is as it was changed: the overview undoes it.
  expect(await order(page, MESSAGES)).toEqual(['Words', 'Links from newcomers']);
  await undo(page).click();
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Links from newcomers']);
});

// ---------------------------------------------------------------------------
// What changed
// ---------------------------------------------------------------------------

// US-G7
test('Apply shows every change before it is sent, each marked, under its section', async ({ page }) => {
  await open(page, editorUrl({ mode: 'dry', rules: [BANNED, NEWCOMERS, SPAM_NAMES] }));
  await expect(page.getByRole('button', { name: 'Apply changes', exact: true })).toBeVisible();
  await mode(page, 'Notifications').check();
  await row(page, 'Banned words').getByRole('switch').click();
  await openRule(page, 'Links from newcomers');
  await rename(page, 'Links from new members');
  await toGroup(page);
  await newRule(page);
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  await page.getByRole('dialog', { name: 'Start from a template' }).getByRole('button', { name: /^🌊 Flood/ }).click();
  await toGroup(page);
  // The button counts the changes: the mode, the switch, the name, the new rule.
  await expect(page.getByRole('button', { name: 'Apply 4 changes', exact: true })).toBeVisible();
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('3 → 4 rules');
  await expect(dialog).toContainText('🛡 Mode: Dry — only tell me what it would do → Notifications — act and tell me');
  await expect(dialog.getByRole('heading', { name: MESSAGES })).toBeVisible();
  await expect(dialog).toContainText('turned off');
  await expect(dialog).toContainText('renamed');
  await expect(dialog).toContainText('was “Links from newcomers”');
  await expect(dialog).toContainText('added');
  await expect(dialog).toContainText('Unchanged: Spam names');
  await looks(page, 'apply-dialog');
});

// US-G7
test('a rule that moved shows its old place', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS, FLOOD)));
  await menuOf(page, 'Flood').click();
  await choose(page, 'Move up').click();
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('moved');
  await expect(dialog).toContainText(/was (2nd|3rd)/);
  await looks(page, 'apply-moved');
});

// US-G7
test('a rule whose When changed shows under its new section, with its old one', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS)));
  await openRule(page, 'Banned words');
  await page.getByRole('radio', { name: '🚪 Someone joins' }).check();
  await toGroup(page);
  const dialog = await applyDialog(page);
  await expect(dialog.getByRole('heading', { name: JOINS })).toBeVisible();
  await expect(dialog).toContainText('changed');
  await expect(dialog).toContainText('was 💬 When someone sends a message');
  await looks(page, 'apply-changed-when');
});

// US-G7
test('a list that lost entries is shown, and a deleted rule is removed', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, FLOOD)));
  await openRule(page, 'Banned words');
  await page.getByRole('button', { name: 'Remove casino' }).click();
  await toGroup(page);
  await menuOf(page, 'Flood').click();
  await choose(page, 'Delete').click();
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('2 → 1 rules');
  await expect(dialog).toContainText('removed');
  await looks(page, 'apply-lost-entries');
});

// US-G7
test('with no change, the dialog says so', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED)));
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('Nothing has changed since you opened this page. You can still copy the link and send it again.');
  await looks(page, 'apply-no-change');
});

// US-G7
test('an incomplete rule cannot be sent: the dialog says what it lacks and offers its page', async ({ page }) => {
  await emptyEditor(page);
  await newRule(page);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await page.keyboard.press('Escape');
  await toGroup(page);
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('“New rule” has no condition yet. Finish it or delete it before sending.');
  await expect(dialog.getByRole('button', { name: 'Copy the link' })).toHaveCount(0);
  await looks(page, 'apply-incomplete-condition');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  // Without an action.
  await emptyEditor(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await page.keyboard.press('Escape');
  await toGroup(page);
  const lacking = await applyDialog(page);
  await expect(lacking).toContainText('“New rule” has no action yet. Finish it or delete it before sending.');
  await lacking.getByRole('button', { name: /^Open “New rule”/ }).click();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('New rule');
});

// US-G7
test('Apply warns when the link nears what one message carries', async ({ page }) => {
  // Words that do not compress: 1,100 of them make a link of about 11,700 bytes, 1,700 one of about 17,700.
  const word = (i: number) => {
    let x = (i * 2654435761) >>> 0;
    let s = '';
    for (let k = 0; k < 8; k++) {
      s += 'abcdefghijklmnopqrstuvwxyz'[x % 26];
      x = (x * 1103515245 + 12345) >>> 0;
    }
    return s + i;
  };
  const long = (n: number) => Array.from({ length: n }, (_, i) => word(i));
  await open(page, editorUrl(rules(named('Long', 'message_posted', msg(long(1100)), MODERATE))));
  expect(await textOf(await applyDialog(page))).toMatch(/This link is 1\d,\d{3} bytes, (7\d|8\d|9\d)% of what a single chat message can carry\. Once it passes 15,000, the bot will no longer be able to read it\./);
  await looks(page, 'size-warning');
  await open(page, editorUrl(rules(named('Long', 'message_posted', msg(long(1700)), MODERATE))));
  expect(await textOf(await applyDialog(page))).toMatch(/This link is 1\d,\d{3} bytes — past the 15,000 a single chat message can carry\. The bot will not be able to read it\. Shorten a long list before sending\./);
  await looks(page, 'size-over');
});

// US-G7: "a reorder inside All or Any is not a change" cannot be played: the editor has no way to
// reorder the conditions inside one.

// US-G8, US-C1
test('the copied link, sent to the bot, saves the changes', async ({ page }) => {
  const s = await scene(rules(BANNED));
  await open(page, await currentLink(s));
  await mode(page, 'Dry').check();
  const dialog = await applyDialog(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  await expect(dialog).toContainText('Link copied. Paste it into your chat with the bot and send it — that is what applies the changes.');
  await expect(dialog.getByRole('button', { name: 'Close' })).toBeVisible();
  await looks(page, 'link-copied');
  const copied = await clipboard(page);
  expect(copied).toMatch(new RegExp(`^\\[Settings for Crypto Chat\\]\\(${EDITOR.replace(/[.]/g, '\\.')}/?#bot_id=${s.id}&group=Crypto%20Chat&config=\\S+\\)$`));
  expect(await s.owner.ask(copied)).toBe('✅ Group settings updated.');
  await s.owner.send('/groups');
  expect(configOf(await s.owner.message()).mode).toBe('dry');
  // From then on, the changes are counted from what was copied.
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(await applyDialog(page)).toContainText('Nothing has changed since you opened this page.');
});

// US-G8
test('a link without a group name is copied as the settings for the group by its number', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED), '7', null));
  await savedConfig(page);
  expect(await clipboard(page)).toMatch(new RegExp(`^\\[Settings for group 7\\]\\(${EDITOR.replace(/[.]/g, '\\.')}/?#bot_id=7&config=\\S+\\)$`));
});

// US-G8
test('the editor says so when it cannot copy', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => Promise.reject(new Error('denied')), readText: () => Promise.reject(new Error('denied')) } });
  });
  await open(page, editorUrl(rules(BANNED)));
  const dialog = await applyDialog(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  await expect(dialog.getByText('Could not reach the clipboard.')).toBeVisible();
  expect(await textOf(dialog)).toContain('Could not reach the clipboard. Copy the whole address from the address bar and send that to the bot.');
  await looks(page, 'clipboard-failed');
});

// ---------------------------------------------------------------------------
// Edit with AI
// ---------------------------------------------------------------------------

const AI_ALL = (page: Page) => page.getByRole('dialog', { name: 'Edit your settings with AI' });
const reply = (config: unknown) => '```json\n' + JSON.stringify(config) + '\n```';

async function editAll(page: Page) {
  await page.getByRole('button', { name: 'More', exact: true }).click();
  await choose(page, 'Edit all with AI').click();
}

// US-G9
test('Edit all with AI: copy the prompt, paste the answer', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS)));
  await editAll(page);
  await expect(AI_ALL(page).getByRole('heading', { name: 'Copy the question' })).toBeVisible();
  await expect(AI_ALL(page).getByRole('heading', { name: 'Paste what it answers' })).toBeVisible();
  // Its footer: Cancel, and what loads the answer, beside it.
  await expect(AI_ALL(page).getByRole('button', { name: 'Cancel', exact: true })).toBeVisible();
  await looks(page, 'ai-all-dialog');
  await AI_ALL(page).getByRole('button', { name: 'Copy the prompt' }).click();
  await expect(page.getByText('Prompt copied. Paste it into your AI chat.')).toBeVisible();
  const prompt = await clipboard(page);
  expect(prompt).toContain('"keywords"');
  expect(prompt).toContain('Banned words');
  const box = AI_ALL(page).getByRole('textbox', { name: 'Paste what it answers' });
  const load = AI_ALL(page).getByRole('button', { name: 'Load into the editor', exact: true });
  await load.click();
  await expect(AI_ALL(page)).toContainText("Paste the AI's reply first.");
  await box.fill('I cannot help with that.');
  await load.click();
  await expect(AI_ALL(page)).toContainText("No settings found in that text. Copy the AI's whole reply, or just its code block.");
  await box.fill(reply(rules({ actions: [], condition: msg(['a']) })));
  await load.click();
  await expect(AI_ALL(page)).toContainText("Not loaded — the AI's settings have problems:");
  await looks(page, 'ai-all-problems');
  await box.fill(reply(rules(
    { ...BANNED, condition: msg(['spam', 'casino', 'scam']) }, NEWCOMERS, FLOOD,
  )));
  await load.click();
  await expect(AI_ALL(page)).toContainText('Loaded 3 rules — 1 changed, 1 added. Press Apply changes when you are ready; you will see exactly what changed before anything is sent.');
  await looks(page, 'ai-all-loaded');
  await AI_ALL(page).getByRole('button', { name: 'Undo the load' }).click();
  await expect(AI_ALL(page)).toContainText('Reverted to the settings and rules you had before pasting.');
  await AI_ALL(page).getByRole('button', { name: 'Cancel', exact: true }).click();
  await expect(row(page, 'Flood')).toHaveCount(0);
  // After a later edit the load cannot be undone from here.
  await editAll(page);
  await box.fill(reply(rules(BANNED, NEWCOMERS, FLOOD)));
  await load.click();
  await AI_ALL(page).getByRole('button', { name: 'Cancel', exact: true }).click();
  await mode(page, 'Silent').check();
  await editAll(page);
  await AI_ALL(page).getByRole('button', { name: 'Undo the load' }).click();
  await expect(page.getByText('You have edited the settings since. Use Undo in the header to step back.')).toBeVisible();
});

// US-G10
test('Edit with AI: long lists and keys stay out of the prompt', async ({ page }) => {
  const key = 'sk-or-v1-secret-a1b2';
  const ai = { type: 'FlaggedByOpenRouterInstruction', api_key: key, model: 'google/gemini-2.5-flash-lite', instruction: 'Match ads.', context_messages: 0, max_attempts: 3, retry_delay_seconds: 1 };
  const config = rules(named('Long', 'message_posted', msg(words(150)), MODERATE), named('Ads', 'message_posted', ai, MODERATE));
  await open(page, editorUrl(config));
  await editAll(page);
  await expect(AI_ALL(page).getByText('1 long list sent as a placeholder')).toBeVisible();
  await AI_ALL(page).getByRole('button', { name: 'Copy the prompt' }).click();
  const prompt = await clipboard(page);
  expect(prompt).toContain('<<KEEP LIST 1: 150 ENTRIES>>');
  expect(prompt).toContain('<<KEEP SECRET 1>>');
  expect(prompt).not.toContain(key);
  const box = AI_ALL(page).getByRole('textbox', { name: 'Paste what it answers' });
  const load = AI_ALL(page).getByRole('button', { name: 'Load into the editor', exact: true });
  await box.fill(reply(rules(named('Long', 'message_posted', msg(['<<KEEP LIST 1: 140 ENTRIES>>']), MODERATE), named('Ads', 'message_posted', { ...ai, api_key: '<<KEEP SECRET 1>>' }, MODERATE))));
  await load.click();
  await expect(AI_ALL(page)).toContainText('the AI wrote 140 where your list has 150 entries — it looks like it edited the list blind');
  await box.fill(reply(rules(named('Long', 'message_posted', msg(['<<KEEP LIST 1: 150 ENTRIES>>']), MODERATE), named('Ads', 'message_posted', { ...ai, api_key: 'sk-made-up' }, MODERATE))));
  await load.click();
  await expect(AI_ALL(page)).toContainText('the AI wrote its own value into the secret field “api_key”. Secrets never go through the AI');
  // Ticked, the list goes in full.
  await AI_ALL(page).getByText('1 long list sent as a placeholder').click();
  await looks(page, 'ai-long-lists');
  await AI_ALL(page).getByRole('checkbox').first().check();
  await AI_ALL(page).getByRole('button', { name: 'Copy the prompt' }).click();
  expect(await clipboard(page)).toContain('word149');
  // Two long lists.
  await open(page, editorUrl(rules(named('A', 'message_posted', msg(words(150)), MODERATE), named('B', 'message_posted', msg(words(120)), MODERATE))));
  await editAll(page);
  await expect(AI_ALL(page).getByText('2 long lists sent as a placeholder')).toBeVisible();
});

// US-G10
test('a long list stays out of a rule prompt too', async ({ page }) => {
  await open(page, editorUrl(rules(named('Long', 'message_posted', msg(words(150)), MODERATE))));
  await openRule(page, 'Long');
  await page.getByRole('button', { name: '✨ Edit with AI' }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit “Long” with AI' });
  await expect(dialog.getByText('1 long list sent as a placeholder')).toBeVisible();
  await dialog.getByRole('button', { name: 'Copy the prompt' }).click();
  expect(await clipboard(page)).toContain('<<KEEP LIST 1: 150 ENTRIES>>');
});

// US-G11
test('import and export JSON, from the overview menu', async ({ page }) => {
  const key = 'sk-or-v1-secret-a1b2';
  const ai = { type: 'FlaggedByOpenRouterInstruction', api_key: key, model: 'google/gemini-2.5-flash-lite', instruction: 'Match ads.', context_messages: 0, max_attempts: 3, retry_delay_seconds: 1 };
  await open(page, editorUrl(rules(named('Ads', 'message_posted', ai, MODERATE))));
  await page.getByRole('button', { name: 'More', exact: true }).click();
  await choose(page, 'Import JSON').click();
  const imp = page.getByRole('dialog', { name: 'Import settings from JSON' });
  await expect(imp.getByRole('button', { name: 'Choose JSON file' })).toBeVisible();
  const box = imp.getByRole('textbox');
  const load = imp.getByRole('button', { name: 'Load settings' });
  await load.click();
  await expect(imp).toContainText('Paste the JSON or choose a file first.');
  await box.fill('not json');
  await load.click();
  await expect(imp).toContainText('That is not valid JSON.');
  await box.fill('{"mode":"dry"}');
  await load.click();
  await expect(imp).toContainText('The rules list is missing.');
  await looks(page, 'import-problem');
  await box.fill(JSON.stringify(rules(BANNED, FLOOD, named('Ads', 'message_posted', ai, MODERATE))));
  await load.click();
  await expect(page.getByText('Loaded the settings and 3 rules. Press Apply changes to send.')).toBeVisible();
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Flood', 'Ads']);
  await undo(page).click();
  expect(await order(page, MESSAGES)).toEqual(['Ads']);
  await page.getByRole('button', { name: 'More', exact: true }).click();
  await choose(page, 'Export JSON').click();
  const exp = page.getByRole('dialog', { name: 'Export settings as JSON' });
  await expect(exp.getByRole('checkbox', { name: 'Clear secret values (API keys)' })).toBeChecked();
  await looks(page, 'export-dialog');
  await exp.getByRole('button', { name: 'Copy JSON' }).click();
  await expect(page.getByText('Settings copied as JSON.')).toBeVisible();
  expect(JSON.parse(await clipboard(page)).rules[0].condition.api_key).toBe('');
  await exp.getByRole('checkbox', { name: 'Clear secret values (API keys)' }).uncheck();
  const download = page.waitForEvent('download');
  await exp.getByRole('button', { name: 'Download JSON' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('moderation-settings.json');
  await expect(page.getByText('Settings exported.')).toBeVisible();
  // The browser is in Docker: the file comes back to the tests by saveAs.
  const path = test.info().outputPath('moderation-settings.json');
  await file.saveAs(path);
  const saved = JSON.parse(require('fs').readFileSync(path, 'utf8'));
  expect(saved.rules[0].condition.api_key).toBe(key);
});

// ---------------------------------------------------------------------------
// The order of the rules, and turning one off
// ---------------------------------------------------------------------------

// US-G12
test("a rule moves from its ⋯ menu: Move up and Move down, none where there is nowhere to go", async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS, FLOOD, SPAM_NAMES)));
  await menuOf(page, 'Banned words').click();
  await expect(page.getByRole('menuitem')).toHaveText(['Move down', 'Disable', 'Delete']);
  await page.keyboard.press('Escape');
  await menuOf(page, 'Flood').click();
  await expect(page.getByRole('menuitem')).toHaveText(['Move up', 'Disable', 'Delete']);
  await looks(page, 'rule-menu');
  await choose(page, 'Move up').click();
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Flood', 'Links from newcomers']);
  await menuOf(page, 'Banned words').click();
  await choose(page, 'Move down').click();
  expect(await order(page, MESSAGES)).toEqual(['Flood', 'Banned words', 'Links from newcomers']);
  // A rule is only ever moved within its section.
  expect(await order(page, JOINS)).toEqual(['Spam names']);
  await menuOf(page, 'Spam names').click();
  await expect(page.getByRole('menuitem')).toHaveText(['Disable', 'Delete']);
});

// US-G12
test('a rule moves by its handle, with the arrow keys and by dragging', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS, FLOOD)));
  const grip = (name: string) => page.getByRole('button', { name: `Move “${name}”`, exact: true });
  await grip('Banned words').focus();
  await page.keyboard.press('ArrowDown');
  expect(await order(page, MESSAGES)).toEqual(['Links from newcomers', 'Banned words', 'Flood']);
  await grip('Banned words').focus();
  await page.keyboard.press('ArrowUp');
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Links from newcomers', 'Flood']);
  // Dragged down past the rule below it.
  const box = (await grip('Banned words').boundingBox())!;
  const below = (await grip('Links from newcomers').boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, below.y + 40, { steps: 10 });
  await page.mouse.up();
  expect(await order(page, MESSAGES)).toEqual(['Links from newcomers', 'Banned words', 'Flood']);
  // The order is a change, and a change that can be undone.
  await undo(page).click();
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Links from newcomers', 'Flood']);
});

// US-G12
test("the ⋯ menu's Delete removes the rule, as a change that can be undone", async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, FLOOD)));
  await menuOf(page, 'Flood').click();
  await choose(page, 'Delete').click();
  expect(await order(page, MESSAGES)).toEqual(['Banned words']);
  await undo(page).click();
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Flood']);
});

// US-G13
test('a rule is turned off with its switch, or in its menu, and turned on again', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, FLOOD)));
  const flood = page.getByRole('switch', { name: 'Flood', exact: true });
  await expect(flood).toBeChecked();
  await flood.click();
  await expect(flood).not.toBeChecked();
  await expect(row(page, 'Flood')).toContainText('Off');
  await looks(page, 'rule-off');
  // Its page says so, with a way to turn it on.
  await openRule(page, 'Flood');
  await expect(page.getByText('⏸ This rule is off — the bot skips it.')).toBeVisible();
  await looks(page, 'rule-off-page');
  await page.getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.getByText('⏸ This rule is off — the bot skips it.')).toHaveCount(0);
  await toGroup(page);
  await expect(flood).toBeChecked();
  // The menu says Disable, then Enable.
  await menuOf(page, 'Flood').click();
  await choose(page, 'Disable').click();
  await expect(flood).not.toBeChecked();
  await menuOf(page, 'Flood').click();
  await choose(page, 'Enable').click();
  await expect(flood).toBeChecked();
});

// US-G13
test('a rule that is off is sent, kept by the bot and not checked', async ({ page }) => {
  const s = await scene(rules(named('Banned words', 'message_posted', msg(['spam']), MODERATE)));
  await open(page, await currentLink(s));
  await page.getByRole('switch', { name: 'Banned words', exact: true }).click();
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('turned off');
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  expect(await s.owner.ask(await clipboard(page))).toBe('✅ Group settings updated.');
  // The bot does not check it...
  await s.member.post(s.memberGroup, 'spam');
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
  // ...but keeps it: the next link has it, off.
  await s.owner.send('/groups');
  const got = configOf(await s.owner.message());
  expect(got.rules).toHaveLength(1);
  expect(got.rules[0].enabled).toBe(false);
  await open(page, await currentLink(s));
  await expect(page.getByRole('switch', { name: 'Banned words', exact: true })).not.toBeChecked();
});

// ---------------------------------------------------------------------------
// A new rule
// ---------------------------------------------------------------------------

// US-G14
test('+ Add rule opens the New rule page, with three ways to start, and nothing is added until one is picked', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED)));
  await newRule(page);
  await expect(page.getByRole('button', { name: 'Back to rules' })).toBeVisible();
  await expect(page.getByText('When someone sends a message', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Start from a template/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Create with AI/ })).toBeVisible();
  await expect(page.getByRole('button', { name: /^Build it myself/ })).toBeVisible();
  await looks(page, 'new-rule-messages');
  await toGroup(page);
  expect(await order(page, MESSAGES)).toEqual(['Banned words']);
  // For when someone joins.
  await newRule(page, 'joins');
  await expect(page.getByText('When someone joins', { exact: true })).toBeVisible();
  await looks(page, 'new-rule-joins');
  await page.goBack();
  expect(await order(page, JOINS)).toEqual([]);
});

// US-G14
test('Build it myself adds a rule with nothing in it, which is incomplete until it has what the bot needs', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED)));
  await newRule(page);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  // Its page opens with the condition types to pick from.
  await expect(page.getByRole('dialog', { name: 'Pick a condition' })).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page.getByText('Pick a condition and at least one action — the bot takes no rule without them.')).toBeVisible();
  await looks(page, 'rule-incomplete');
  await toGroup(page);
  await expect(row(page, 'New rule')).toContainText('Incomplete');
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'New rule']);
  await looks(page, 'overview-incomplete');
  // Given a condition and an action, it is complete.
  await openRule(page, 'New rule');
  await page.getByRole('button', { name: 'Add condition', exact: true }).click();
  await pickable(page, 'Message Is Empty or Blank').click();
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  await pickable(page, 'Moderate the message').click();
  await expect(page.getByText('Pick a condition and at least one action — the bot takes no rule without them.')).toHaveCount(0);
  await toGroup(page);
  // With nothing to call it by, it is called by what it does.
  await expect(page.getByRole('button', { name: /^IF is empty or blank/ })).toBeVisible();
  await expect(page.getByText('Incomplete', { exact: true })).toHaveCount(0);
});

// US-G14
test('a rule for when someone joins needs only an action', async ({ page }) => {
  await emptyEditor(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await page.keyboard.press('Escape');
  await expect(page.getByText('Pick at least one action — the bot takes no rule without one.')).toBeVisible();
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  await pickable(page, 'Kick the member').click();
  await expect(page.getByText('Pick at least one action — the bot takes no rule without one.')).toHaveCount(0);
});

// US-G15
test('Start from a template opens a sheet with the templates of the trigger', async ({ page }) => {
  await emptyEditor(page);
  await newRule(page);
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  const sheet = page.getByRole('dialog', { name: 'Start from a template' });
  for (const [title, then] of [
    ['🚫 Banned words', 'THEN 🗑 moderate'],
    ['🔗 Links from newcomers', 'THEN 🗑 moderate'],
    ['🌊 Flood', 'THEN 👁 observer'],
    ['📏 Walls of text', 'THEN 🗑 moderate'],
  ]) {
    const item = sheet.getByRole('button', { name: new RegExp(`^${title}`) });
    await expect(item).toContainText('IF ');
    await expect(item).toContainText(then);
  }
  await expect(sheet.getByRole('button', { name: 'Cancel', exact: true })).toBeVisible();
  await looks(page, 'templates-sheet-messages');
  await sheet.getByRole('button', { name: 'Cancel', exact: true }).click();
  await toGroup(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  const joins = page.getByRole('dialog', { name: 'Start from a template' });
  await expect(joins.getByRole('button', { name: /^👁 Quarantine newcomers/ })).toContainText('THEN 👁 observer');
  await expect(joins.getByRole('button', { name: /^🚫 Spam names/ })).toBeVisible();
  await expect(joins.getByRole('button', { name: /^🔗 Links in names/ })).toBeVisible();
  await expect(joins.getByRole('button', { name: /^🚫 Banned words/ })).toHaveCount(0);
  await looks(page, 'templates-sheet-joins');
});

// US-G15
test('a template is added at the end of its section, named after itself, and opens its page', async ({ page }) => {
  await emptyEditor(page);
  const take = async (trigger: 'messages' | 'joins', title: RegExp) => {
    await newRule(page, trigger);
    await page.getByRole('button', { name: /^Start from a template/ }).click();
    await page.getByRole('dialog', { name: 'Start from a template' }).getByRole('button', { name: title }).click();
  };
  await take('messages', /^🚫 Banned words/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Banned words');
  // Its list is empty, to fill in.
  await expect(page.getByRole('textbox', { name: 'Add to Words / Phrases', exact: true })).toBeFocused();
  await toGroup(page);
  await take('messages', /^🔗 Links from newcomers/);
  await toGroup(page);
  await take('messages', /^🌊 Flood/);
  await toGroup(page);
  await take('messages', /^📏 Walls of text/);
  await toGroup(page);
  // The same template twice: the second has a number.
  await take('messages', /^🚫 Banned words/);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Banned words 2');
  await toGroup(page);
  await take('joins', /^👁 Quarantine newcomers/);
  await toGroup(page);
  await take('joins', /^🚫 Spam names/);
  await toGroup(page);
  await take('joins', /^🔗 Links in names/);
  await toGroup(page);
  expect(await order(page, MESSAGES)).toEqual(['Banned words', 'Links from newcomers', 'Flood', 'Walls of text', 'Banned words 2']);
  expect(await order(page, JOINS)).toEqual(['Quarantine newcomers', 'Spam names', 'Links in names']);
  const saved = (await savedConfig(page)).rules;
  const by = (name: string) => saved.find((r: any) => r.name === name);
  expect(by('Banned words')).toMatchObject({ trigger: 'message_posted', condition: { type: 'ContainsWords', keywords: [] }, actions: [{ type: 'ModerateMessage' }] });
  expect(by('Links from newcomers').condition).toMatchObject({ type: 'All', conditions: [{ type: 'AuthorJoinedRecently', time_window_minutes: 60 }, { type: 'ContainsLinksOutsideTop100' }] });
  expect(by('Flood')).toMatchObject({ condition: { type: 'AuthorHitsMessageRateLimit', message_count: 5, time_window_minutes: 1 }, actions: [{ type: 'SetAuthorObserver', duration_minutes: 10 }] });
  expect(by('Walls of text')).toMatchObject({ condition: { type: 'ExceedsMaxLines', max_lines: 10, chars_per_line: 40 }, actions: [{ type: 'ModerateMessage' }] });
  expect(by('Quarantine newcomers')).toMatchObject({ trigger: 'member_joined', actions: [{ type: 'SetAuthorObserver', duration_minutes: 60 }] });
  expect(by('Quarantine newcomers').condition).toBeUndefined();
  expect(by('Spam names')).toMatchObject({ trigger: 'member_joined', condition: { type: 'MemberNameContainsWords', keywords: [] }, actions: [{ type: 'KickAuthor', delete_all_messages: false }] });
  expect(by('Links in names')).toMatchObject({ trigger: 'member_joined', condition: { type: 'MemberNameContainsLinksOutsideList', domains: [] }, actions: [{ type: 'KickAuthor' }] });
  // Taking one is a change that can be undone.
  await undo(page).click();
  expect(await order(page, JOINS)).toEqual(['Quarantine newcomers', 'Spam names']);
});

// US-G16
test('Create with AI: describe the rule, copy the prompt, paste the answer', async ({ page }) => {
  await emptyEditor(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: /^Create with AI/ }).click();
  const dialog = page.getByRole('dialog', { name: 'Create a rule with AI' });
  await expect(dialog.getByRole('heading', { name: 'What should the rule do?' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Copy the question' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Paste what it answers' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeVisible();
  await looks(page, 'ai-create-dialog');
  // With nothing written.
  await dialog.getByRole('button', { name: 'Copy the prompt' }).click();
  await expect(dialog).toContainText('Describe the rule first.');
  await dialog.getByRole('textbox', { name: 'What should the rule do?' }).fill('Kick anyone whose name advertises a casino');
  await dialog.getByRole('button', { name: 'Copy the prompt' }).click();
  await expect(page.getByText('Prompt copied. Paste it into your AI chat.')).toBeVisible();
  const prompt = await clipboard(page);
  // What it says, and what the trigger takes.
  expect(prompt).toContain('Kick anyone whose name advertises a casino');
  expect(prompt).toContain('MemberNameContainsWords');
  expect(prompt).toContain('KickAuthor');
  expect(prompt).not.toContain('ContainsImage');
  expect(prompt).not.toContain('ModerateMessage');
  const box = dialog.getByRole('textbox', { name: 'Paste what it answers' });
  const add = dialog.getByRole('button', { name: 'Add the rule', exact: true });
  await add.click();
  // Nothing written says so; and what is no rule, a mistake or several rules.
  await box.fill('');
  await add.click();
  await expect(dialog).toContainText("Paste the AI's reply first.");
  await box.fill('I cannot help with that.');
  await add.click();
  await expect(dialog).toContainText("No rule found in that text. Copy the AI's whole reply, or just its code block.");
  await box.fill(reply({ name: 'Bad', actions: [], condition: { type: 'MemberNameContainsWords', keywords: ['casino'] } }));
  await add.click();
  await expect(dialog).toContainText("Not added — the AI's rule has problems:");
  await looks(page, 'ai-create-problems');
  const one = { name: 'Casino names', condition: { type: 'MemberNameContainsWords', keywords: ['casino'] }, actions: KICK };
  await box.fill(reply([one, one, one]));
  await add.click();
  await expect(dialog).toContainText('The answer has 3 rules — ask the AI for one.');
  // A valid rule: added at the end of the section, with the section's trigger; its page opens.
  await box.fill(reply(one));
  await add.click();
  await expect(page.getByText('Added “Casino names”. Press Apply changes when you are ready.')).toBeVisible();
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Casino names');
  await expect(page.getByRole('radio', { name: '🚪 Someone joins' })).toBeChecked();
  await toGroup(page);
  expect(await order(page, JOINS)).toEqual(['Casino names']);
  expect((await savedConfig(page)).rules[0].trigger).toBe('member_joined');
  await undo(page).click();
  expect(await order(page, JOINS)).toEqual([]);
});

// US-G16
test('Create with AI for messages offers the rule only what the trigger takes, and refuses what does not fit', async ({ page }) => {
  await emptyEditor(page);
  await newRule(page);
  await page.getByRole('button', { name: /^Create with AI/ }).click();
  const dialog = page.getByRole('dialog', { name: 'Create a rule with AI' });
  await dialog.getByRole('textbox', { name: 'What should the rule do?' }).fill('Delete images');
  await dialog.getByRole('button', { name: 'Copy the prompt' }).click();
  const prompt = await clipboard(page);
  expect(prompt).toContain('ContainsImage');
  expect(prompt).toContain('ModerateMessage');
  const box = dialog.getByRole('textbox', { name: 'Paste what it answers' });
  await box.fill(reply({ name: 'Images', condition: { type: 'ContainsImage' }, actions: MODERATE }));
  await dialog.getByRole('button', { name: 'Add the rule', exact: true }).click();
  await expect(page.getByText('Added “Images”. Press Apply changes when you are ready.')).toBeVisible();
});

// US-G17
test('Edit with AI on a rule: its prompt holds this rule alone, and the answer replaces it and nothing else', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, NEWCOMERS)));
  await openRule(page, 'Banned words');
  await page.getByRole('button', { name: '✨ Edit with AI' }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit “Banned words” with AI' });
  await expect(dialog.getByRole('heading', { name: 'Copy the question' })).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Paste what it answers' })).toBeVisible();
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeVisible();
  await looks(page, 'ai-rule-dialog');
  await dialog.getByRole('button', { name: 'Copy the prompt' }).click();
  await expect(page.getByText('Prompt copied. Paste it into your AI chat.')).toBeVisible();
  const prompt = await clipboard(page);
  expect(prompt).toContain('"spam"');
  // Nothing else of the configuration goes into it.
  expect(prompt).not.toContain('Links from newcomers');
  expect(prompt).not.toContain('"mode"');
  const box = dialog.getByRole('textbox', { name: 'Paste what it answers' });
  const load = dialog.getByRole('button', { name: 'Load into the rule', exact: true });
  await load.click();
  await expect(dialog).toContainText("Paste the AI's reply first.");
  await box.fill('I cannot help with that.');
  await load.click();
  await expect(dialog).toContainText("No rule found in that text. Copy the AI's whole reply, or just its code block.");
  const changed = { name: 'Banned words', condition: msg(['spam', 'casino', 'scam']), actions: MODERATE };
  // Several rules, or a whole configuration, are more than this rule.
  await box.fill(reply([changed, changed, changed]));
  await load.click();
  await expect(dialog).toContainText('The answer has 3 rules — ask the AI for this one rule only.');
  await box.fill(reply(rules(changed, changed, changed)));
  await load.click();
  await expect(dialog).toContainText('The answer has 3 rules — ask the AI for this one rule only.');
  await box.fill(reply({ name: 'Banned words', actions: [], condition: msg(['x']) }));
  await load.click();
  await expect(dialog).toContainText("Not loaded — the AI's rule has problems:");
  await looks(page, 'ai-rule-problems');
  await box.fill(reply(changed));
  await load.click();
  await expect(dialog).toContainText('Updated “Banned words”. Press Apply changes when you are ready.');
  await looks(page, 'ai-rule-loaded');
  await dialog.getByRole('button', { name: 'Undo the load' }).click();
  await expect(dialog).toContainText('Reverted to the rule you had before pasting.');
  await box.fill(reply(changed));
  await load.click();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  const saved = await savedConfig(page);
  expect(saved.rules[0].condition.keywords).toEqual(['spam', 'casino', 'scam']);
  // Nothing else changed.
  expect(saved.rules[1]).toEqual(NEWCOMERS);
  // After a later edit the load cannot be undone from here.
  await openRule(page, 'Banned words');
  await page.getByRole('button', { name: '✨ Edit with AI' }).click();
  await box.fill(reply({ ...changed, condition: msg(['again']) }));
  await load.click();
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await rename(page, 'Words');
  await page.getByRole('button', { name: '✨ Edit with AI' }).click();
  await page.getByRole('dialog', { name: 'Edit “Words” with AI' }).getByRole('button', { name: 'Undo the load' }).click();
  await expect(page.getByText('You have edited the settings since. Use Undo in the header to step back.')).toBeVisible();
});

// ---------------------------------------------------------------------------
// The demo
// ---------------------------------------------------------------------------

const DEMO_BANNER = "🧪 This is a demo. Everything works as it will for your own group: add rules, change them, try the AI. Nothing here reaches any group. To moderate yours, invite the bot to it as moderator or higher, and the link it sends you opens this editor on your group's rules.";
const DEMO_NOTE = "This is a demo, so there is nothing to send. Invite the bot to your group, and the link it sends you opens this editor on your group's rules.";
const banner = (page: Page) => page.getByRole('note');

// US-G18
test('the link of the welcome opens a demo group with no rules, in the mode Notifications, under a banner', async ({ page }) => {
  await open(page, EDITOR);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Demo group');
  await expect(page.getByText('Moderation settings · Moderator Bot', { exact: true })).toBeVisible();
  await expect(banner(page)).toHaveText(DEMO_BANNER);
  await expect(mode(page, 'Notifications')).toBeChecked();
  await expect(section(page, MESSAGES)).toContainText('No rules yet. Nothing is moderated until you add one.');
  await expect(section(page, JOINS)).toContainText('No rules yet. Nobody is checked when they join.');
  await expect(page.getByRole('button', { name: 'Apply changes', exact: true })).toBeVisible();
  await expect(page.getByText('No bot ID found in the URL.')).toHaveCount(0);
  await looks(page, 'demo');
});

// US-G18
test('the demo banner is above the New rule page and a rule\'s page too', async ({ page }) => {
  await open(page, EDITOR);
  await newRule(page);
  await expect(banner(page)).toHaveText(DEMO_BANNER);
  await looks(page, 'demo-new-rule');
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  await page.getByRole('dialog', { name: 'Start from a template' }).getByRole('button', { name: /^🚫 Banned words/ }).click();
  await expect(page.getByRole('heading', { name: 'Banned words', level: 1 })).toBeVisible();
  await expect(banner(page)).toHaveText(DEMO_BANNER);
});

// US-G18
test('the demo works as the editor of a group: a rule can be added, and Apply counts it', async ({ page }) => {
  await open(page, EDITOR);
  await newRule(page);
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  await page.getByRole('dialog', { name: 'Start from a template' }).getByRole('button', { name: /^🌊 Flood/ }).click();
  await toGroup(page);
  await expect(row(page, 'Flood')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Apply 1 change', exact: true })).toBeVisible();
  await expect(undo(page)).toBeVisible();
});

// US-G18
test('Apply with no change in the demo says there is nothing to send, and has no Copy the link', async ({ page }) => {
  await open(page, EDITOR);
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('Nothing has changed since you opened this page.');
  await expect(dialog).not.toContainText('You can still copy the link');
  await expect(dialog).toContainText(DEMO_NOTE);
  await expect(dialog.getByRole('button', { name: 'Copy the link' })).toHaveCount(0);
  await looks(page, 'demo-apply-no-change');
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();
});

// US-G18
test('Apply with changes in the demo lists them as for a group, and has no Copy the link', async ({ page }) => {
  await open(page, EDITOR);
  await mode(page, 'Dry').check();
  await newRule(page);
  await page.getByRole('button', { name: /^Start from a template/ }).click();
  await page.getByRole('dialog', { name: 'Start from a template' }).getByRole('button', { name: /^🌊 Flood/ }).click();
  await toGroup(page);
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('0 → 1 rules');
  await expect(dialog).toContainText('🛡 Mode: Notifications — act and tell me → Dry — only tell me what it would do');
  await expect(dialog.getByRole('heading', { name: MESSAGES })).toBeVisible();
  await expect(dialog).toContainText('added');
  await expect(dialog).toContainText(DEMO_NOTE);
  await expect(dialog.getByRole('button', { name: 'Copy the link' })).toHaveCount(0);
  await looks(page, 'demo-apply-changed');
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(dialog).toBeHidden();
});

// US-G18: a rule that is not finished is refused as for a group (US-G7).
test('Apply with a rule not finished in the demo names the rule, as for a group', async ({ page }) => {
  await open(page, EDITOR);
  await newRule(page);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await page.keyboard.press('Escape');
  await toGroup(page);
  const dialog = await applyDialog(page);
  await expect(dialog).toContainText('“New rule” has no condition yet. Finish it or delete it before sending.');
  await expect(dialog.getByRole('button', { name: 'Copy the link' })).toHaveCount(0);
});

// US-G18
test('a link with settings but no bot id opens the demo on those settings', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED, SPAM_NAMES), null));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Demo group');
  await expect(banner(page)).toHaveText(DEMO_BANNER);
  await expect(row(page, 'Banned words')).toBeVisible();
  await expect(row(page, 'Spam names')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Apply changes', exact: true })).toBeVisible();
});

// US-G18
test('a link without a bot id whose settings cannot be read opens the demo, with the note', async ({ page }) => {
  await open(page, `${EDITOR}/#config=garbage`);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Demo group');
  await expect(banner(page)).toHaveText(DEMO_BANNER);
  await expect(page.getByText('The settings in this link could not be read. Starting from scratch.')).toBeVisible();
});

// US-G18
test('a group\'s own link opens no demo', async ({ page }) => {
  await open(page, editorUrl(rules(BANNED)));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Crypto Chat');
  await expect(banner(page)).toHaveCount(0);
  const dialog = await applyDialog(page);
  await expect(dialog.getByRole('button', { name: 'Copy the link' })).toBeVisible();
  await expect(dialog).not.toContainText(DEMO_NOTE);
});
