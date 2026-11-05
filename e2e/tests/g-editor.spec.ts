// G. The web editor, as the owner uses it in a browser.
import { expect, Locator, Page, test } from '@playwright/test';
import LZ from 'lz-string';
import { configOf } from '../harness/person';
import { EDITOR } from '../harness/stack';
import { editorUrl, emptyEditor, rule, rules, scene } from './helpers';

test.use({ permissions: ['clipboard-read', 'clipboard-write'] });

const STEPS = `1️⃣ Choose the group's mode. 2️⃣ Pick a rule in the list, or add a new one, and for each rule set the 🔎 Condition — what to detect — and the ⚡ Actions — what the bot does when a message matches. 3️⃣ When you're done, press "Apply changes" and send the copied link back to the bot.`;
const words = (n: number) => Array.from({ length: n }, (_, i) => `word${i}`);
const W = (keywords: string[]) => rule({ type: 'ContainsWords', keywords });
const pane = (page: Page) => page.getByRole('region', { name: 'Rule', exact: true });
const clipboard = (page: Page) => page.evaluate(() => navigator.clipboard.readText());

/** Opens `url` afresh: going to another link that differs only by its hash would not reload the editor. */
async function open(page: Page, url: string) {
  await page.goto('about:blank');
  await page.goto(url);
}

/** What a person reads in `locator`, its whitespace as the browser shows it. */
async function textOf(locator: Locator): Promise<string> {
  return (await locator.innerText()).replace(/\s+/g, ' ');
}

async function apply(page: Page) {
  await page.getByRole('button', { name: 'Apply changes' }).click();
  return page.getByRole('dialog', { name: 'Changes before you send them' });
}

// US-G1
test('the editor shows the group settings and the rules', async ({ page }) => {
  const s = await scene(rules(W(['a']), W(['b']), W(['c'])));
  await s.owner.send('/groups');
  await open(page, (await s.owner.message()).match(/\((http[^)]+)\)/)![1]);
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('Moderation settings for Moderator Bot');
  await expect(page.getByText(STEPS)).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Group settings' })).toBeVisible();
  await expect(page.getByRole('combobox', { name: '🛡 Mode' })).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Rules' })).toBeVisible();
  await expect(page.getByText('3 total')).toBeVisible();
  await expect(page.getByRole('button', { name: /^\d IF / })).toHaveCount(3);
  await emptyEditor(page);
  await expect(page.getByText('No rules yet. Nothing is moderated until you add one.')).toBeVisible();
});

// US-G1
test('a rule of a type the editor does not know is kept as it is', async ({ page }) => {
  const unknown = { actions: [{ type: 'ModerateMessage' }], condition: { type: 'FutureType', level: 3 } };
  await open(page, editorUrl(rules(unknown, W(['a']))));
  const type = pane(page).getByRole('combobox', { name: 'Condition type' });
  await expect(type.locator('option:checked')).toHaveText('FutureType (unknown)');
  await expect(pane(page).getByRole('spinbutton')).toHaveCount(0);
  await pane(page).getByRole('button', { name: 'ⓘ' }).nth(1).click();
  await expect(pane(page).getByText("This type is not in this editor's schema. It is kept as it is — leave it alone unless you mean to replace it.")).toBeVisible();
  await page.getByRole('combobox', { name: '🛡 Mode' }).selectOption({ label: 'Dry — only tell me what it would do' });
  await (await apply(page)).getByRole('button', { name: 'Copy the link' }).click();
  expect(configOf(await clipboard(page)).rules[0]).toEqual(unknown);
});

// US-G1
test('a link without a bot id, or with settings that cannot be read', async ({ page }) => {
  await open(page, editorUrl(rules(W(['a'])), null));
  await expect(page.getByText('No bot ID found in the URL. Open the link the bot sent you in a direct message — a link opened by hand cannot be applied.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Apply changes' })).toBeHidden();
  await open(page, `${EDITOR}/#bot_id=7&config=garbage`);
  await expect(page.getByText('The settings in this link could not be read. Starting from scratch.')).toBeVisible();
  await expect(page.getByText('0 total')).toBeVisible();
});

// US-G2
test('the mode is chosen in group settings', async ({ page }) => {
  await emptyEditor(page);
  const mode = page.getByRole('combobox', { name: '🛡 Mode' });
  await expect(mode.locator('option')).toHaveText(['Notifications — act and tell me', 'Silent — act without telling me', 'Dry — only tell me what it would do']);
  await mode.selectOption({ label: 'Silent — act without telling me' });
  await (await apply(page)).getByRole('button', { name: 'Copy the link' }).click();
  expect(configOf(await clipboard(page)).mode).toBe('silent');
});

// US-G3
test('a rule is added from conditions grouped by subject', async ({ page }) => {
  await emptyEditor(page);
  await page.getByRole('button', { name: '+ Add rule' }).click();
  const type = pane(page).getByRole('combobox', { name: 'Condition type' });
  await expect(type.locator('optgroup')).toHaveCount(7);
  for (const group of ['Message text', 'Links', 'Size & shape', 'Attachments', 'Author', 'Group']) {
    await expect(type.locator(`optgroup[label="${group}"]`)).toHaveCount(1);
  }
  await type.selectOption({ label: 'Message rate limit' });
  await expect(pane(page).getByRole('spinbutton', { name: 'Number of Messages' })).toHaveValue('5');
  await expect(pane(page).getByRole('spinbutton', { name: 'Time Window (Minutes)' })).toHaveValue('1');
  await pane(page).getByRole('button', { name: 'ⓘ' }).nth(1).click();
  await expect(pane(page).getByText('👤 Author Hits Message Rate Limit', { exact: false }).first()).toBeVisible();
});

// US-G3
test('a list is edited entry by entry, filtered, or as text', async ({ page }) => {
  await open(page, editorUrl(rules(W([]), W(words(150)))));
  const add = pane(page).getByRole('textbox', { name: 'add + Enter' });
  await add.fill('spam');
  await add.press('Enter');
  await expect(pane(page).getByRole('button', { name: 'Remove spam' })).toBeVisible();
  await add.fill('spam');
  await add.press('Enter');
  await expect(page.getByText('That entry is already in the list.')).toBeVisible();
  await pane(page).getByRole('button', { name: 'Remove spam' }).click();
  await expect(pane(page).getByRole('button', { name: 'Remove spam' })).toHaveCount(0);
  await page.getByRole('button', { name: /^2 IF / }).click();
  const filter = pane(page).getByRole('textbox', { name: 'Filter Words / Phrases' });
  await expect(filter).toHaveAttribute('placeholder', 'filter 150 entries');
  await filter.fill('word149');
  await expect(pane(page).getByRole('button', { name: /^Remove word/ })).toHaveCount(1);
  await pane(page).getByRole('button', { name: 'bulk edit' }).click();
  await expect(pane(page).getByText('One per line; applied when you click away — this is how a long list gets pasted in, or replaced wholesale. Up to 10000 entries, 100 characters each.')).toBeVisible();
  await pane(page).getByRole('button', { name: 'chips' }).click();
  await expect(pane(page).getByRole('textbox', { name: 'add + Enter' })).toBeVisible();
});

// US-G4
test('changing a type keeps what both types have, and a composite unwraps', async ({ page }) => {
  await open(page, editorUrl(rules(
    rule({ type: 'All', conditions: [{ type: 'ContainsWords', keywords: ['a'] }, { type: 'ContainsWords', keywords: ['b'] }] }),
    rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }),
    rule({ type: 'Any', conditions: [{ type: 'All', conditions: [{ type: 'ContainsWords', keywords: ['x'] }, { type: 'IsBlank' }] }, { type: 'ContainsWords', keywords: ['y'] }] }),
  )));
  await pane(page).getByRole('combobox', { name: 'Condition type' }).first().selectOption({ label: 'Any of these (OR)' });
  await expect(pane(page).getByRole('combobox', { name: 'Condition type' })).toHaveCount(3);
  await page.getByRole('button', { name: /^2 IF / }).click();
  await pane(page).getByRole('combobox', { name: 'Condition type' }).first().selectOption({ label: 'Link outside this list' });
  await expect(pane(page).getByRole('button', { name: 'Remove spam.example' })).toBeVisible();
  await page.getByRole('button', { name: /^3 IF / }).click();
  await pane(page).getByRole('button', { name: '⋮' }).nth(1).click();
  await pane(page).getByRole('button', { name: 'Unwrap' }).click();
  await expect(pane(page).getByRole('combobox', { name: 'Condition type' })).toHaveCount(4);
  await (await apply(page)).getByRole('button', { name: 'Copy the link' }).click();
  const saved = configOf(await clipboard(page));
  expect(saved.rules[0].condition.type).toBe('Any');
  expect(saved.rules[0].condition.conditions).toHaveLength(2);
  expect(saved.rules[1].condition).toEqual({ type: 'ContainsLinksOutsideList', domains: ['spam.example'] });
  expect(saved.rules[2].condition.conditions).toHaveLength(3);
});

// US-G5
test('the editor warns past the limits, and marks a covered action', async ({ page }) => {
  const tree = (nots: number, leaves: number): unknown =>
    nots === 0 ? { type: 'All', conditions: Array.from({ length: leaves }, (_, i) => ({ type: 'ContainsWords', keywords: [`w${i}`] })) } : { type: 'Not', condition: tree(nots - 1, leaves) };
  await open(page, editorUrl(rules(
    rule(tree(7, 62)),
    rule({ type: 'IsBlank' }, [{ type: 'ModerateMessage' }, { type: 'KickAuthor', delete_all_messages: true }]),
  )));
  await expect(pane(page).getByText('This rule has 70 conditions nested 9 levels deep. The bot accepts at most 64 conditions and 8 levels, so simplify it before applying — otherwise the rules will be rejected.')).toBeVisible();
  await page.getByRole('button', { name: /^2 IF / }).click();
  const covered = pane(page).getByText('covered', { exact: true });
  await expect(covered).toBeVisible();
  await expect(covered).toHaveAttribute('title', 'A stronger action on this rule already covers it');
});

// US-G6
test('every change can be undone and redone', async ({ page }) => {
  await emptyEditor(page);
  const undo = page.getByRole('button', { name: 'Undo' });
  const redo = page.getByRole('button', { name: 'Redo' });
  await expect(undo).toBeHidden();
  await expect(redo).toBeHidden();
  const mode = page.getByRole('combobox', { name: '🛡 Mode' });
  await mode.selectOption({ label: 'Dry — only tell me what it would do' });
  await undo.click();
  await expect(mode).toHaveValue(/c0|notifications/);
  await expect(redo).toBeVisible();
  await expect(undo).toBeHidden();
  await redo.click();
  await expect(mode.locator('option:checked')).toHaveText('Dry — only tell me what it would do');
  await page.locator('body').click();
  await page.keyboard.press('Control+z');
  await expect(mode.locator('option:checked')).toHaveText('Notifications — act and tell me');
  await page.keyboard.press('Control+Shift+z');
  await expect(mode.locator('option:checked')).toHaveText('Dry — only tell me what it would do');
  // Up to 20 steps.
  for (let i = 0; i < 25; i++) await page.getByRole('button', { name: '+ Add rule' }).click();
  for (let i = 0; i < 20; i++) await undo.click();
  await expect(undo).toBeHidden();
  await expect(page.getByText('5 total')).toBeVisible();
});

// US-G7
test('apply changes shows every change before it is sent', async ({ page }) => {
  await open(page, editorUrl({ mode: 'dry', rules: [W(['a']), W(['b'])] }));
  await page.getByRole('combobox', { name: '🛡 Mode' }).selectOption({ label: 'Notifications — act and tell me' });
  await page.getByRole('button', { name: '+ Add rule' }).click();
  const dialog = await apply(page);
  await expect(dialog).toContainText('2 → 3 rules');
  await expect(dialog).toContainText('🛡 Mode: Dry — only tell me what it would do → Notifications — act and tell me');
  await expect(dialog).toContainText('added');
  await expect(dialog).toContainText('Unchanged rules: 1, 2');
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await open(page, editorUrl(rules(W(['a']))));
  const none = await apply(page);
  await expect(none).toContainText('Nothing has changed since you opened this page. You can still copy the link and send it again.');
});

// US-G7
test('apply changes warns when the link nears what one message carries', async ({ page }) => {
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
  await open(page, editorUrl(rules(W(long(1100)))));
  expect(await textOf(await apply(page))).toMatch(/This link is 1\d,\d{3} bytes, (7\d|8\d|9\d)% of what a single chat message can carry\. Once it passes 15,000, the bot will no longer be able to read it\./);
  await open(page, editorUrl(rules(W(long(1700)))));
  expect(await textOf(await apply(page))).toMatch(/This link is 1\d,\d{3} bytes — past the 15,000 a single chat message can carry\. The bot will not be able to read it\. Shorten a long list before sending\./);
});

// US-G8, US-C1
test('the copied link, sent to the bot, saves the changes', async ({ page }) => {
  const s = await scene(rules(W(['spam'])));
  await open(page, s.link);
  await page.getByRole('combobox', { name: '🛡 Mode' }).selectOption({ label: 'Dry — only tell me what it would do' });
  const dialog = await apply(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  expect(await textOf(dialog)).toContain('Link copied. Paste it into your chat with the bot and send it — that is what applies the changes.');
  await expect(dialog.getByRole('button', { name: 'Close' })).toBeVisible();
  const copied = await clipboard(page);
  expect(copied).toMatch(new RegExp(`^\\[Settings for group ${s.id}\\]\\(${EDITOR.replace(/[.]/g, '\\.')}/?#bot_id=${s.id}&config=\\S+\\)$`));
  expect(await s.owner.ask(copied)).toBe('✅ Group settings updated.');
  await s.owner.send('/groups');
  expect(configOf(await s.owner.message()).mode).toBe('dry');
  // From then on, the changes are counted from what was copied.
  await dialog.getByRole('button', { name: 'Close' }).click();
  await expect(await apply(page)).toContainText('Nothing has changed since you opened this page.');
});

// US-G8
test('the editor says so when it cannot copy', async ({ browser }) => {
  const context = await browser.newContext({ permissions: [] });
  const page = await context.newPage();
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: { writeText: () => Promise.reject(new Error('denied')), readText: () => Promise.reject(new Error('denied')) } });
  });
  await open(page, editorUrl(rules(W(['a']))));
  const dialog = await apply(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  await expect(dialog.getByText('Could not reach the clipboard.')).toBeVisible();
  expect(await textOf(dialog)).toContain('Could not reach the clipboard. Copy the whole address from the address bar and send that to the bot.');
  await context.close();
});

const AI = (page: Page) => page.getByRole('dialog', { name: 'Edit your settings with AI' });
const reply = (config: unknown) => '```json\n' + JSON.stringify(config) + '\n```';

// US-G9
test('edit with AI: copy the prompt, paste the answer', async ({ page }) => {
  await open(page, editorUrl(rules(W(['a']), W(['b']))));
  await page.getByRole('button', { name: 'Edit with AI' }).click();
  await expect(AI(page).getByRole('heading', { name: 'Copy the question' })).toBeVisible();
  await AI(page).getByRole('button', { name: 'Copy the prompt' }).click();
  await expect(page.getByText('Prompt copied. Paste it into your AI chat.')).toBeVisible();
  expect(await clipboard(page)).toContain('"keywords"');
  await expect(AI(page).getByRole('heading', { name: 'Paste what it answers' })).toBeVisible();
  const box = AI(page).getByRole('textbox', { name: "Paste the AI's reply here" });
  const load = AI(page).getByRole('button', { name: 'Load into the editor' });
  await load.click();
  await expect(AI(page)).toContainText("Paste the AI's reply first.");
  await box.fill('I cannot help with that.');
  await load.click();
  await expect(AI(page)).toContainText("No settings found in that text. Copy the AI's whole reply, or just its code block.");
  await box.fill(reply(rules({ actions: [], condition: { type: 'ContainsWords', keywords: ['a'] } })));
  await load.click();
  await expect(AI(page)).toContainText("Not loaded — the AI's settings have problems:");
  await box.fill(reply(rules(W(['a', 'z']), W(['b']), W(['c']))));
  await load.click();
  await expect(AI(page)).toContainText('Loaded 3 rules — 1 changed, 1 added. Press Apply changes when you are ready; you will see exactly what changed before anything is sent.');
  await AI(page).getByRole('button', { name: 'Undo the load' }).click();
  await expect(AI(page)).toContainText('Reverted to the settings and rules you had before pasting.');
  await expect(page.getByText('2 total')).toBeVisible();
  await box.fill(reply(rules(W(['a']), W(['b']), W(['c']))));
  await load.click();
  await AI(page).getByRole('button', { name: 'Close' }).last().click();
  await page.getByRole('button', { name: '+ Add rule' }).click();
  await page.getByRole('button', { name: 'Edit with AI' }).click();
  await AI(page).getByRole('button', { name: 'Undo the load' }).click();
  await expect(page.getByText('You have edited the settings since. Use Undo in the header to step back.')).toBeVisible();
});

// US-G10
test('edit with AI: long lists and keys stay out of the prompt', async ({ page }) => {
  const key = 'sk-or-v1-secret-a1b2';
  const ai = { type: 'FlaggedByOpenRouterInstruction', api_key: key, model: 'google/gemini-2.5-flash-lite', instruction: 'Match ads.', context_messages: 0, max_attempts: 3, retry_delay_seconds: 1 };
  const config = rules(W(words(150)), rule(ai));
  await open(page, editorUrl(config));
  await page.getByRole('button', { name: 'Edit with AI' }).click();
  await expect(AI(page).getByText('1 long list sent as a placeholder')).toBeVisible();
  await AI(page).getByRole('button', { name: 'Copy the prompt' }).click();
  const prompt = await clipboard(page);
  expect(prompt).toContain('<<KEEP LIST 1: 150 ENTRIES>>');
  expect(prompt).toContain('<<KEEP SECRET 1>>');
  expect(prompt).not.toContain(key);
  const box = AI(page).getByRole('textbox', { name: "Paste the AI's reply here" });
  const load = AI(page).getByRole('button', { name: 'Load into the editor' });
  await box.fill(reply(rules(W(['<<KEEP LIST 1: 140 ENTRIES>>']), rule({ ...ai, api_key: '<<KEEP SECRET 1>>' }))));
  await load.click();
  await expect(AI(page)).toContainText('the AI wrote 140 where your list has 150 entries — it looks like it edited the list blind');
  await box.fill(reply(rules(W(['<<KEEP LIST 1: 150 ENTRIES>>']), rule({ ...ai, api_key: 'sk-made-up' }))));
  await load.click();
  await expect(AI(page)).toContainText('the AI wrote its own value into the secret field “api_key”. Secrets never go through the AI');
  // Ticked, the list goes in full.
  await AI(page).getByText('1 long list sent as a placeholder').click();
  await AI(page).getByRole('checkbox').first().check();
  await AI(page).getByRole('button', { name: 'Copy the prompt' }).click();
  expect(await clipboard(page)).toContain('word149');
  // Two long lists.
  await open(page, editorUrl(rules(W(words(150)), W(words(120)))));
  await page.getByRole('button', { name: 'Edit with AI' }).click();
  await expect(AI(page).getByText('2 long lists sent as a placeholder')).toBeVisible();
});

// US-G11
test('import and export JSON', async ({ page }) => {
  const key = 'sk-or-v1-secret-a1b2';
  const ai = { type: 'FlaggedByOpenRouterInstruction', api_key: key, model: 'google/gemini-2.5-flash-lite', instruction: 'Match ads.', context_messages: 0, max_attempts: 3, retry_delay_seconds: 1 };
  await open(page, editorUrl(rules(rule(ai))));
  await page.getByRole('button', { name: 'Import JSON' }).click();
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
  await box.fill(JSON.stringify(rules(W(['a']), W(['b']), rule(ai))));
  await load.click();
  await expect(page.getByText('Loaded the settings and 3 rules. Press Apply changes to send.')).toBeVisible();
  await expect(page.getByText('3 total')).toBeVisible();
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('1 total')).toBeVisible();
  await page.getByRole('button', { name: 'Export JSON' }).click();
  const exp = page.getByRole('dialog', { name: 'Export settings as JSON' });
  await expect(exp.getByRole('checkbox', { name: 'Clear secret values (API keys)' })).toBeChecked();
  await exp.getByRole('button', { name: 'Copy JSON' }).click();
  await expect(page.getByText('Settings copied as JSON.')).toBeVisible();
  expect(JSON.parse(await clipboard(page)).rules[0].condition.api_key).toBe('');
  await page.getByRole('button', { name: 'Export JSON' }).click();
  await exp.getByRole('checkbox', { name: 'Clear secret values (API keys)' }).uncheck();
  const download = page.waitForEvent('download');
  await exp.getByRole('button', { name: 'Download JSON' }).click();
  const file = await download;
  expect(file.suggestedFilename()).toBe('moderation-settings.json');
  await expect(page.getByText('Settings exported.')).toBeVisible();
  const saved = JSON.parse(require('fs').readFileSync(await file.path(), 'utf8'));
  expect(saved.rules[0].condition.api_key).toBe(key);
});

void LZ;
