// F. Actions.
import { expect, test } from '@playwright/test';
import { Person, configOf } from '../harness/person';
import { advance } from '../harness/stack';
import { emptyEditor, joinNotice, joinRule, looks, newRule, notice, pickable, rule, rules, scene } from './helpers';

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
  expect(await s.owner.message()).toBe(notice('moderated the message', 'spam', ["contains word: 'spam'"]));
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
  expect(await s.owner.message()).toBe(notice('set the member as observer for 1 minute', 'brief', ["contains word: 'brief'"]));
  // The message stays; the member is a member again when the minute is up.
  advance(2);
  expect(await s.member.seesInGroup(120_000)).toEqual({ kind: 'role', role: 'member' });
  await s.member.post(s.memberGroup, 'forever');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the member as observer', 'forever', ["contains word: 'forever'"]));
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
  expect(await keeps.owner.message()).toBe(notice('kicked the member', 'kick me', ["contains word: 'kick'"]));
  // The message stays: the owner still sees it.
  expect(await keeps.owner.history(keeps.ownerGroup)).toContainEqual({ text: 'kick me', deleted: false });
  // With the history: the owner sees every message of the member's deleted.
  const wipes = await scene(rules(rule({ type: 'ContainsWords', keywords: ['kick'] }, [{ type: 'KickAuthor', delete_all_messages: true }])));
  await wipes.member.post(wipes.memberGroup, 'an earlier message');
  await wipes.member.seesNothing(3000);
  await wipes.member.post(wipes.memberGroup, 'kick me');
  expect(await wipes.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await wipes.owner.message()).toBe(notice('kicked the member and deleted all their messages', 'kick me', ["contains word: 'kick'"]));
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
  expect(await s.owner.message()).toBe(notice('set the member as observer for 60 minutes and moderated the message', 'spam at https://spam.example', ["contains word: 'spam'", 'spam.example']));
  // A kick that deletes every message covers Moderate the message.
  const u = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'KickAuthor', delete_all_messages: true }]),
  ));
  await u.member.post(u.memberGroup, 'spam');
  expect(await u.owner.message()).toBe(notice('kicked the member and deleted all their messages', 'spam', ["contains word: 'spam'", "contains word: 'spam'"]));
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
