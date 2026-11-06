// F. Actions.
import { expect, test } from '@playwright/test';
import { configOf } from '../harness/person';
import { advance } from '../harness/stack';
import { emptyEditor, notice, rule, rules, scene } from './helpers';

async function actionFields(page: import('@playwright/test').Page, type: string) {
  await emptyEditor(page);
  await page.getByRole('button', { name: '+ Add rule' }).click();
  const pane = page.getByRole('region', { name: 'Rule', exact: true });
  await pane.getByRole('combobox', { name: 'Action' }).first().selectOption({ label: type });
  return pane;
}

// US-F1
test('moderate the message', async ({ page }) => {
  const pane = await actionFields(page, 'Moderate the message');
  await expect(pane.getByRole('spinbutton')).toHaveCount(0);
  await expect(pane.getByRole('checkbox')).toHaveCount(0);
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] })), 'moderator');
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(notice('moderated the message', 'spam', ["contains word: 'spam'"]));
});

// US-F2
test('set author as observer, for a while or for good', async ({ page }) => {
  const pane = await actionFields(page, 'Set author as observer');
  const field = pane.getByRole('spinbutton', { name: 'Restore author to member after (minutes)' });
  await expect(field).toHaveValue('0');
  await expect(field).toHaveAttribute('max', '43200');
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['brief'] }, [{ type: 'SetAuthorObserver', duration_minutes: 1 }]),
    rule({ type: 'ContainsWords', keywords: ['forever'] }, [{ type: 'SetAuthorObserver', duration_minutes: 0 }]),
  ), 'admin');
  await s.member.post(s.memberGroup, 'brief');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the author as observer for 1 minute', 'brief', ["contains word: 'brief'"]));
  // The message stays; the author is a member again when the minute is up.
  advance(2);
  expect(await s.member.seesInGroup(120_000)).toEqual({ kind: 'role', role: 'member' });
  await s.member.post(s.memberGroup, 'forever');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('set the author as observer', 'forever', ["contains word: 'forever'"]));
  advance(10);
  await s.member.seesNothing(90_000);
});

// US-F3
test('kick the author, with or without their history', async ({ page }) => {
  const pane = await actionFields(page, 'Kick the author');
  await expect(pane.getByRole('checkbox', { name: 'Also delete every message the author has ever sent in this group' })).not.toBeChecked();
  const keeps = await scene(rules(rule({ type: 'ContainsWords', keywords: ['kick'] }, [{ type: 'KickAuthor', delete_all_messages: false }])));
  await keeps.member.post(keeps.memberGroup, 'kick me');
  expect(await keeps.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await keeps.owner.message()).toBe(notice('kicked the author', 'kick me', ["contains word: 'kick'"]));
  // The message stays: the owner still sees it.
  expect(await keeps.owner.history(keeps.ownerGroup)).toContainEqual({ text: 'kick me', deleted: false });
  // With the history: the owner sees every message of the author's deleted.
  const wipes = await scene(rules(rule({ type: 'ContainsWords', keywords: ['kick'] }, [{ type: 'KickAuthor', delete_all_messages: true }])));
  await wipes.member.post(wipes.memberGroup, 'an earlier message');
  await wipes.member.seesNothing(3000);
  await wipes.member.post(wipes.memberGroup, 'kick me');
  expect(await wipes.member.seesInGroup()).toEqual({ kind: 'removed' });
  expect(await wipes.owner.message()).toBe(notice('kicked the author and deleted all their messages', 'kick me', ["contains word: 'kick'"]));
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
  expect(await s.owner.message()).toBe(notice('set the author as observer for 60 minutes and moderated the message', 'spam at https://spam.example', ["contains word: 'spam'", 'spam.example']));
  // A kick that deletes every message covers Moderate the message.
  const u = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'ModerateMessage' }]),
    rule({ type: 'ContainsWords', keywords: ['spam'] }, [{ type: 'KickAuthor', delete_all_messages: true }]),
  ));
  await u.member.post(u.memberGroup, 'spam');
  expect(await u.owner.message()).toBe(notice('kicked the author and deleted all their messages', 'spam', ["contains word: 'spam'", "contains word: 'spam'"]));
  // A configuration saved is merged the same way.
  const t = await scene(rules(rule({ type: 'ContainsWords', keywords: ['x'] }, [
    { type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 5 }, { type: 'SetAuthorObserver', duration_minutes: 30 },
  ])));
  await t.owner.send('/groups');
  const saved = configOf(await t.owner.message());
  expect(saved.rules[0].actions).toEqual([{ type: 'SetAuthorObserver', duration_minutes: 30 }, { type: 'ModerateMessage' }]);
});
