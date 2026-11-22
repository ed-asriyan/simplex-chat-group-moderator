// F. Actions.
import { expect, test } from '@playwright/test';
import { Person, configOf } from '../harness/person';
import { advance } from '../harness/stack';
import { anotherMember, applyDialog, currentLink, emptyEditor, joinNotice, joinRule, lineEnds, looks, message, named, newRule, notice, open, pickable, rule, rules, scene, toGroup, turnedOff } from './helpers';

/** A new rule's page with the action `title` added: the page is all the owner has to look at. */
async function actionEditor(page: import('@playwright/test').Page, title: string, trigger: 'messages' | 'joins' = 'messages') {
  await emptyEditor(page);
  await newRule(page, trigger);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  // The condition's list is not what this is about: the action is.
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  await pickable(page, title).click();
  await looks(page, `action-${title.toLowerCase().replace(/[^a-z0-9]+/g, '-')}`);
}

// US-F1
test('moderate the message', async ({ page }) => {
  await actionEditor(page, 'Moderate the message');
  await expect(page.getByRole('spinbutton')).toHaveCount(0);
  await expect(page.getByRole('checkbox')).toHaveCount(0);
  // A rule for when someone joins does not offer it.
  await emptyEditor(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: 'Build it myself' }).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  const list = page.getByRole('dialog', { name: 'Pick an action' });
  await expect(list.getByRole('button', { name: /^Set the member as observer\s/ })).toBeVisible();
  await expect(list.getByRole('button', { name: /^Kick the member\s/ })).toBeVisible();
  await expect(list.getByRole('button', { name: /^Moderate the message\s/ })).toHaveCount(0);
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] })), 'moderator');
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(notice(s.member, 'moderated the message', 'spam', ["contains word: 'spam'"]));
});

// US-F2
test('set the member as observer, for a while or for good', async ({ page }) => {
  await actionEditor(page, 'Set the member as observer');
  const field = page.getByRole('spinbutton', { name: 'Restore them to member after (minutes)' });
  await expect(field).toHaveValue('0');
  await expect(field).toHaveAttribute('max', '43200');
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['brief'] }, [{ type: 'SetAuthorObserver', duration_minutes: 1 }]),
    rule({ type: 'ContainsWords', keywords: ['forever'] }, [{ type: 'SetAuthorObserver', duration_minutes: 0 }]),
  ), 'admin');
  await s.member.post(s.memberGroup, 'brief');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice(s.member, 'set the member as observer for 1 minute', 'brief', ["contains word: 'brief'"]));
  // The message stays; the member is a member again when the minute is up.
  advance(2);
  expect(await s.member.seesInGroup(120_000)).toEqual({ kind: 'role', role: 'member' });
  await s.member.post(s.memberGroup, 'forever');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice(s.member, 'set the member as observer', 'forever', ["contains word: 'forever'"]));
  advance(10);
  await s.member.seesNothing(90_000);
});

// US-F3
test('kick the member, with or without their history', async ({ page }) => {
  await actionEditor(page, 'Kick the member');
  await expect(page.getByRole('checkbox', { name: 'Also delete every message the member has ever sent in this group' })).not.toBeChecked();
  const keeps = await scene(rules(rule({ type: 'ContainsWords', keywords: ['kick'] }, [{ type: 'KickAuthor', delete_all_messages: false }])));
  await keeps.member.post(keeps.memberGroup, 'kick me');
  expect(await keeps.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await keeps.owner.message()).toBe(notice(keeps.member, 'kicked the member', 'kick me', ["contains word: 'kick'"]));
  // The message stays: the owner still sees it.
  expect(await keeps.owner.history(keeps.ownerGroup)).toContainEqual({ text: 'kick me', deleted: false });
  // With the history: the owner sees every message of the member's deleted.
  const wipes = await scene(rules(rule({ type: 'ContainsWords', keywords: ['kick'] }, [{ type: 'KickAuthor', delete_all_messages: true }])));
  await wipes.member.post(wipes.memberGroup, 'an earlier message');
  await wipes.member.seesNothing(3000);
  await wipes.member.post(wipes.memberGroup, 'kick me');
  expect(await wipes.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await wipes.owner.message()).toBe(notice(wipes.member, 'kicked the member and deleted all their messages', 'kick me', ["contains word: 'kick'"]));
  await expect
    .poll(() => wipes.owner.history(wipes.ownerGroup), { timeout: 60_000 })
    .toEqual(expect.arrayContaining([{ text: 'an earlier message', deleted: true }, { text: 'kick me', deleted: true }]));
});

// US-F4
test('overlapping actions are done once, in order', async () => {
  // A longer observer time covers a shorter one; the rest run observer, moderate, kick.
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 10 }]),
    rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }, [{ type: 'SetAuthorObserver', duration_minutes: 60 }]),
  ));
  await s.member.post(s.memberGroup, 'spam at https://spam.example');
  expect(await s.owner.message()).toBe(notice(s.member, 'set the member as observer for 60 minutes and moderated the message', 'spam at https://spam.example', ["contains word: 'spam'", 'spam.example']));
  // A kick that deletes every message covers Moderate the message.
  const u = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'KickAuthor', delete_all_messages: true }]),
  ));
  await u.member.post(u.memberGroup, 'spam');
  expect(await u.owner.message()).toBe(notice(u.member, 'kicked the member and deleted all their messages', 'spam', ["contains word: 'spam'", "contains word: 'spam'"]));
  // A configuration saved is merged the same way.
  const t = await scene(rules(rule({ type: 'ContainsWords', keywords: ['x'] }, [
    { type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 5 }, { type: 'SetAuthorObserver', duration_minutes: 30 },
  ])));
  await t.owner.send('/groups');
  const saved = configOf(await t.owner.message());
  expect(saved.rules[0].actions).toEqual([{ type: 'SetAuthorObserver', duration_minutes: 30 }, { type: 'ModerateMessage' }]);
});

// US-F2
test('the member who joined is made an observer, and restored', async () => {
  const s = await scene(rules(joinRule(undefined, [{ type: 'SetAuthorObserver', duration_minutes: 1 }], 'Quarantine newcomers')), 'admin');
  const dave = await Person.create('Dave');
  await dave.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await dave.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(joinNotice('set the member as observer for 1 minute', dave.name, ['**Quarantine newcomers**: everyone who joins']));
  advance(2);
  expect(await dave.seesInGroup(120_000)).toEqual({ kind: 'role', role: 'member' });
});

// US-F3
test('the member who joined is kicked, with their history or without', async () => {
  const s = await scene(rules(joinRule({ type: 'MemberNameContainsWords', keywords: ['casino'] }, [{ type: 'KickAuthor', delete_all_messages: true }], 'Spam names')));
  const bot = await Person.create('Free Casino Bonus');
  await bot.joinGroupAndWatch(await s.owner.groupLink(s.ownerGroup));
  expect(await bot.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(joinNotice('kicked the member and deleted all their messages', bot.name, ["**Spam names**: name contains word: 'casino'"]));
});

// ---- ✉️ Send the member a private message ----

const WARNING = 'Your message was removed: no spam here. Please read the pinned rules.';
const SPAM = { type: 'ContainsWords', keywords: ['spam'] };

// US-F5
test('send the member a private message: an empty Message of several lines, up to 1000 characters, in rules for messages only', async ({ page }) => {
  await actionEditor(page, 'Send the member a private message');
  const field = page.getByRole('textbox', { name: 'Message', exact: true });
  await expect(field).toHaveValue('');
  await expect(field).toHaveAttribute('maxlength', '1000');
  // Several lines: a line break stays in it.
  await field.fill('First line\nSecond line');
  await expect(field).toHaveValue('First line\nSecond line');
  // A rule for when someone joins does not offer it.
  await emptyEditor(page);
  await newRule(page, 'joins');
  await page.getByRole('button', { name: 'Build it myself' }).click();
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  const list = page.getByRole('dialog', { name: 'Pick an action' });
  await expect(list.getByRole('button', { name: /^Kick the member\s/ })).toBeVisible();
  await expect(list.getByRole('button', { name: /^Send the member a private message\s/ })).toHaveCount(0);
});

// US-F5
test('the message is sent exactly as the owner wrote it, in SimpleX Markdown', async () => {
  const text = '**Read the rules** first.\n_No spam_, please: {name} stays as written.';
  const s = await scene(rules(rule(SPAM, [message(text)])));
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text });
});

// US-F6
test('the member whose message breaks a rule gets the message in the chat with admins, before it is moderated', async () => {
  const s = await scene(rules(named('Banned words', 'message_posted', SPAM, [message(WARNING), { type: 'ModerateMessage' }])), 'moderator');
  const item = await s.member.post(s.memberGroup, 'cheap spam here');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately and moderated the message', 'cheap spam here', ["**Banned words**: contains word: 'spam'"]));
  await s.member.seesNothing(3000);
});

// US-F6
test('a warning alone: the member is told, the message stays, and nobody else in the group sees it', async () => {
  const warning = 'Links to other websites are not allowed here.';
  const s = await scene(rules(named('No links', 'message_posted', { type: 'ContainsLinksOutsideList', domains: ['simplex.chat'] }, [message(warning)])), 'moderator');
  const carol = await anotherMember(s);
  await s.member.post(s.memberGroup, 'look at https://spam.example');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: warning });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately', 'look at https://spam.example', ['**No links**: spam.example']));
  // Nothing else is done, and Carol, a member too, sees nothing of it: not in the chat with admins, not in the group.
  const posted = ['look at https://spam.example'];
  await s.member.seesNothing(3000);
  await carol.person.seesNothing(3000, posted);
  expect(await s.owner.history(s.ownerGroup)).toEqual([{ text: posted[0], deleted: false }]);
  // A message the rule does not match gets nothing.
  const item = await s.member.post(s.memberGroup, 'see https://simplex.chat/blog');
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
  // Edited into one it matches, it is checked again, and the member is told again.
  await s.member.edit(s.memberGroup, item, 'see https://spam.example');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: warning });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately', 'see https://spam.example', ['**No links**: spam.example']));
  await carol.person.seesNothing(3000, [...posted, 'see https://simplex.chat/blog']);
  expect((await s.owner.history(s.ownerGroup)).map((m) => m.text)).toEqual([...posted, 'see https://spam.example']);
});

// US-F6
test('a rule without a private message to the member sends nothing to anyone, whatever else it does', async () => {
  const a = await scene(rules(rule(SPAM, [{ type: 'SetAuthorObserver', duration_minutes: 10 }, { type: 'ModerateMessage' }])), 'admin');
  const carolA = await anotherMember(a);
  const item = await a.member.post(a.memberGroup, 'spam');
  expect(await a.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await a.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  await a.member.seesNothing(4000);
  // Carol sees the message moderated, as everyone does, and nothing more.
  expect(await carolA.person.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [expect.any(Number)] });
  await carolA.person.seesNothing(3000, ['spam']);
  expect((await a.owner.history(a.ownerGroup)).map((m) => m.text)).toEqual(['spam']);

  const k = await scene(rules(rule(SPAM, [{ type: 'KickAuthor', delete_all_messages: false }])));
  const carolK = await anotherMember(k);
  await k.member.post(k.memberGroup, 'spam');
  expect(await k.member.seesInGroup()).toEqual({ kind: 'removed' });
  await k.member.seesNothing(4000);
  await carolK.person.seesNothing(3000, ['spam']);
  expect((await k.owner.history(k.ownerGroup)).map((m) => m.text)).toEqual(['spam']);
});

// US-F6
test('the member gets the message before being made an observer', async () => {
  const s = await scene(rules(rule(SPAM, [{ type: 'SetAuthorObserver', duration_minutes: 10 }, message(WARNING)])), 'admin');
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately and set the member as observer for 10 minutes', 'spam', ["contains word: 'spam'"]));
});

// US-F6
test('the member gets the message before being kicked, and before their messages are deleted', async () => {
  const s = await scene(rules(rule(SPAM, [{ type: 'KickAuthor', delete_all_messages: true }, message(WARNING)])));
  await s.member.post(s.memberGroup, 'an earlier message');
  await s.member.seesNothing(3000);
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  expect(await s.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately and kicked the member and deleted all their messages', 'spam', ["contains word: 'spam'"]));
  await expect
    .poll(() => s.owner.history(s.ownerGroup), { timeout: 60_000 })
    .toEqual(expect.arrayContaining([{ text: 'an earlier message', deleted: true }, { text: 'spam', deleted: true }]));
  // Kicked without the history: the message still comes first, and the messages stay.
  const k = await scene(rules(rule(SPAM, [{ type: 'KickAuthor', delete_all_messages: false }, message(WARNING)])));
  await k.member.post(k.memberGroup, 'spam');
  expect(await k.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  expect(await k.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await k.owner.message()).toBe(notice(k.member, 'messaged the member privately and kicked the member', 'spam', ["contains word: 'spam'"]));
  expect(await k.owner.history(k.ownerGroup)).toContainEqual({ text: 'spam', deleted: false });
});

// US-F6, US-F4
test('a message that matches several rules with a message gets the member one, the first matched rule\'s', async () => {
  const s = await scene(rules(
    named('Banned words', 'message_posted', SPAM, [message('From the first rule.'), { type: 'ModerateMessage' }]),
    named('Links', 'message_posted', { type: 'ContainsLinksInList', domains: ['spam.example'] }, [message('From the second rule.'), { type: 'SetAuthorObserver', duration_minutes: 10 }]),
  ), 'admin');
  const item = await s.member.post(s.memberGroup, 'spam at https://spam.example');
  // One message, then the observer and the moderation, in that order.
  expect([await s.member.seesInGroup(), await s.member.seesInGroup(), await s.member.seesInGroup()]).toEqual([
    { kind: 'admins', text: 'From the first rule.' },
    { kind: 'role', role: 'observer' },
    { kind: 'deleted', itemIds: [item] },
  ]);
  expect(await s.owner.message()).toBe(notice(
    s.member,
    'messaged the member privately, set the member as observer for 10 minutes and moderated the message',
    'spam at https://spam.example',
    ["**Banned words**: contains word: 'spam'", '**Links**: spam.example'],
  ));
  await s.member.seesNothing(4000);
});

// US-F6, US-D3
test('with the rule\'s Notifications off, the member still gets the message and the owner is not told', async () => {
  const s = await scene(rules(turnedOff(rule(SPAM, [message(WARNING)]), 'notify')));
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  await s.owner.hearsNothing(4000);
});

// US-F6
test('with the rule\'s Actions off, the member gets nothing', async () => {
  const s = await scene(rules(turnedOff(rule(SPAM, [message(WARNING), { type: 'ModerateMessage' }]), 'enabled')));
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.owner.message()).toBe(notice(s.member, 'would message the member privately and moderate the message', 'spam', ["_(off)_ contains word: 'spam'"]));
  await s.member.seesNothing(5000);
});

// US-F6
test('a message that cannot reach the member: the rule\'s other actions are carried out all the same', async () => {
  const s = await scene(rules(rule(SPAM, [message(WARNING), { type: 'ModerateMessage' }])), 'moderator');
  // The owner turns the group's chat with admins off: nobody can write to Bob there.
  await s.owner.chatWithAdmins(s.ownerGroup, false);
  await s.member.seesNothing(3000);
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  // The notification names the message whether it reached Bob or not (US-D2).
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately and moderated the message', 'spam', ["contains word: 'spam'"]));
  await s.member.seesNothing(3000);
});

// US-F4
test('a private message to the member covers another, and a configuration saved keeps the first, in front', async () => {
  const s = await scene(rules(rule(SPAM, [
    { type: 'ModerateMessage' }, message('First.'), { type: 'SetAuthorObserver', duration_minutes: 5 }, message('Second.'),
  ])), 'admin');
  await s.owner.send('/groups');
  const saved = configOf(await s.owner.message());
  expect(saved.rules[0].actions).toEqual([message('First.'), { type: 'SetAuthorObserver', duration_minutes: 5 }, { type: 'ModerateMessage' }]);
  await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: 'First.' });
  expect(await s.owner.message()).toBe(notice(s.member, 'messaged the member privately, set the member as observer for 5 minutes and moderated the message', 'spam', ["contains word: 'spam'"]));
});

// US-F5, US-G8, US-F6
test('the owner sets the message up in the editor, sends the link, and the member gets it', async ({ page, context }) => {
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  const s = await scene();
  await open(page, await currentLink(s));
  await newRule(page);
  await page.getByRole('button', { name: /^Build it myself/ }).click();
  await pickable(page, 'Message Contains Any of These Words').click();
  const words = page.getByRole('textbox', { name: 'Add to Words / Phrases' });
  await words.fill('spam');
  await words.press('Enter');
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  await pickable(page, 'Send the member a private message').click();
  await page.getByRole('textbox', { name: 'Message', exact: true }).fill(WARNING);
  await page.getByRole('button', { name: 'Add action', exact: true }).click();
  await pickable(page, 'Moderate the message').click();
  await toGroup(page);
  const dialog = await applyDialog(page);
  await dialog.getByRole('button', { name: 'Copy the link' }).click();
  await expect(dialog.getByText('Link copied.')).toBeVisible();
  expect(await s.owner.ask(await page.evaluate(() => navigator.clipboard.readText()))).toBe('✅ Group settings updated.');
  const item = await s.member.post(s.memberGroup, 'cheap spam here');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'admins', text: WARNING });
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(lineEnds(await s.owner.message())).toBe(lineEnds(notice(s.member, 'messaged the member privately and moderated the message', 'cheap spam here', ["contains word: 'spam'"])));
});
