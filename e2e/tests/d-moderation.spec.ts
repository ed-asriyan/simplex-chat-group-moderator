// D. Moderation.
import { expect, test } from '@playwright/test';
import { apply, moderated, notice, rule, rules, scene, untouched } from './helpers';

// US-D1
test('every rule that matches contributes, and an edit is checked again', async () => {
  const s = await scene(rules(
    rule({ type: 'ContainsWords', keywords: ['spam'] }),
    rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }),
  ));
  await moderated(s, 'spam at https://spam.example', ["contains word: 'spam'", 'spam.example']);
  const item = await s.member.post(s.memberGroup, 'hello');
  await s.member.seesNothing(3000);
  await s.member.edit(s.memberGroup, item, 'hello spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toBe(notice('moderated the message', 'hello spam', ["contains word: 'spam'"]));
});

// US-D2
test('notifications mode acts and tells the owner', async () => {
  const s = await scene(rules(rule({ type: 'ContainsLinksInList', domains: ['spam.example'] }, [
    { type: 'ModerateMessage' }, { type: 'SetAuthorObserver', duration_minutes: 60 },
  ])));
  const item = await s.member.post(s.memberGroup, 'Buy cheap followers at spam.example');
  const seen = [await s.member.seesInGroup(), await s.member.seesInGroup()];
  expect(seen).toContainEqual({ kind: 'deleted', itemIds: [item] });
  expect(seen).toContainEqual({ kind: 'role', role: 'observer' });
  expect(await s.owner.message()).toBe(notice('moderated the message and set the author as observer for 60 minutes', 'Buy cheap followers at spam.example', ['spam.example']));
});

// US-D2
test('the notification quotes the message, cut after 300 characters', async () => {
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] }), rule({ type: 'ContainsImage' })));
  const long = `spam ${'x'.repeat(400)}`;
  await moderated(s, long, ["contains word: 'spam'"], `${long.slice(0, 300)}...`);
  await moderated(s, 'first line spam\nsecond line', ["contains word: 'spam'"]);
  await moderated(s, { type: 'image', text: '', image: PIXEL }, ['contains an image'], null);
});

// US-D3
test('silent mode acts without telling', async () => {
  const s = await scene({ mode: 'silent', rules: [rule({ type: 'ContainsWords', keywords: ['spam'] })] });
  const item = await s.member.post(s.memberGroup, 'spam');
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  await s.owner.hearsNothing(5000);
});

// US-D4
test('dry mode only tells what it would do', async () => {
  const dry = (actions: unknown[]) => ({ mode: 'dry', rules: [rule({ type: 'ContainsWords', keywords: ['spam'] }, actions)] });
  const s = await scene(dry([{ type: 'SetAuthorObserver', duration_minutes: 60 }, { type: 'ModerateMessage' }]));
  for (const [actions, did] of [
    [[{ type: 'SetAuthorObserver', duration_minutes: 60 }, { type: 'ModerateMessage' }], 'would set the author as observer for 60 minutes and moderate the message'],
    [[{ type: 'ModerateMessage' }, { type: 'KickAuthor', delete_all_messages: false }], 'would moderate the message and kick the author'],
    [[{ type: 'KickAuthor', delete_all_messages: true }], 'would kick the author and delete all their messages'],
  ] as const) {
    await apply(s, dry([...actions]));
    await s.member.post(s.memberGroup, 'spam');
    expect(await s.owner.message()).toBe(notice(did, 'spam', ["contains word: 'spam'"]));
    // Nothing in the group: the message stays, the author stays a member.
    await s.member.seesNothing(4000);
  }
});

// US-D5
test('a moderator, an admin or an owner is never checked', async () => {
  const s = await scene(rules(rule({ type: 'ContainsWords', keywords: ['spam'] })));
  await s.owner.post(s.ownerGroup, 'spam from the owner');
  await s.owner.seesNothing(5000);
  for (const role of ['moderator', 'admin']) {
    await s.owner.setRole(s.ownerGroup, s.member.name, role);
    expect(await s.member.seesInGroup()).toEqual({ kind: 'role', role });
    await s.member.post(s.memberGroup, `spam from a ${role}`);
    await s.member.seesNothing(5000);
    await s.owner.hearsNothing(0);
  }
});

const PIXEL = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=';
