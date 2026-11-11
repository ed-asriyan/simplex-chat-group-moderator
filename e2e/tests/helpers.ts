// What the stories share: a moderated group with a member in it, the owner's
// notification, and the editor opened on a link.
import { expect, Page } from '@playwright/test';
import { Person, botId, settingsLink } from '../harness/person';

export const ISSUE = 'https://github.com/ed-asriyan/simplex-chat-group-moderator/issues/new?template=moderation-rule-bug.yml';

/** An owner, their group Crypto Chat moderated by the bot, and Bob in it. */
export interface Scene {
  owner: Person;
  member: Person;
  /** The group in the owner's CLI and in Bob's. */
  ownerGroup: number;
  memberGroup: number;
  /** The bot's id for the group, and the editor link it sent. */
  id: string;
  link: string;
}

export async function scene(config?: unknown, role = 'owner'): Promise<Scene> {
  const owner = await Person.join('Alice');
  await owner.message();
  const { group, link } = await owner.moderatedGroup('Crypto Chat', role);
  const member = await Person.create('Bob');
  const memberGroup = await member.joinGroup(await owner.groupLink(group));
  const s = { owner, member, ownerGroup: group, memberGroup, id: botId(link), link };
  if (config) await apply(s, config);
  return s;
}

/** The owner sends the configuration's link, which the bot saves. */
export async function apply(s: Scene, config: unknown) {
  expect(await s.owner.ask(settingsLink(s.id, config))).toBe('✅ Group settings updated.');
}

/** One rule: `condition` and `actions` (by default, moderate the message). */
export function rule(condition: unknown, actions: unknown[] = [{ type: 'ModerateMessage' }]) {
  return { actions, condition };
}

export function rules(...list: unknown[]) {
  return { mode: 'notifications', rules: list };
}

/** The owner's notification of US-D2. */
export function notice(did: string, message: string | null, reasons: string[], group = 'Crypto Chat') {
  // Each line quoted; the story does not say how a line break inside a quote is written.
  const quoted = message === null ? '_(no text)_' : message.split('\n').map((l) => `> ${l}`).join('\n');
  return `🛡 I ${did} in **${group}**.\n\n**The message:**\n${quoted}\n\n**Reason:**\n${reasons.map((r) => `- ${r}`).join('\n')}\n\nIf it's a false positive, please [file an issue](${ISSUE}).`;
}

/** Bob posts `content`; it is moderated, and the owner is told why. */
export async function moderated(s: Scene, content: string | Record<string, unknown>, reasons: string[], message?: string | null) {
  const item = await s.member.post(s.memberGroup, content);
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  const text = message !== undefined ? message : quoted(typeof content === 'string' ? content : (content.text as string) || null);
  expect(lineEnds(await s.owner.message())).toBe(lineEnds(notice('moderated the message', text, reasons)));
}

/** A message as the notification quotes it: cut after 300 characters. */
export function quoted(text: string | null): string | null {
  return text !== null && [...text].length > 300 ? `${[...text].slice(0, 300).join('')}...` : text;
}

/** A notification with the spaces at the ends of its lines left out: the story does not say how a line ends. */
export function lineEnds(text: string): string {
  return text.replace(/ +\n/g, '\n');
}

/** Bob posts `content`; it is moderated, and the owner is told, whatever the reason. */
export async function moderatedAnyway(s: Scene, content: string | Record<string, unknown>) {
  const item = await s.member.post(s.memberGroup, content);
  expect(await s.member.seesInGroup()).toEqual({ kind: 'deleted', itemIds: [item] });
  expect(await s.owner.message()).toMatch(/^🛡 I moderated the message in \*\*Crypto Chat\*\*\.\n/);
}

/** Another member who joins the scene's group. */
export async function anotherMember(s: Scene, name = 'Carol'): Promise<{ person: Person; group: number }> {
  const person = await Person.create(name);
  const group = await person.joinGroup(await s.owner.groupLink(s.ownerGroup));
  return { person, group };
}

/** Bob posts `content`, and nothing happens. */
export async function untouched(s: Scene, content: string | Record<string, unknown>) {
  await s.member.post(s.memberGroup, content);
  await s.member.seesNothing(4000);
  await s.owner.hearsNothing(0);
}

// ---- the editor ----

/** Opens the editor on an empty configuration of the bot's group 7. */
export async function emptyEditor(page: Page) {
  await open(page, editorUrl({ mode: 'notifications', rules: [] }));
}

/**
 * The page as the person sees it now matches its approved screenshot,
 * `tests/__screenshots__/<project>/<spec>/<name>.png` (AGENTS.md, *Workflow*).
 */
export async function looks(page: Page, name: string) {
  await expect(page).toHaveScreenshot(`${name}.png`, { fullPage: true });
}

/** Opens `url` afresh: going to another link that differs only by its hash would not reload the editor. */
export async function open(page: Page, url: string) {
  await page.goto('about:blank');
  await page.goto(url);
}

/** The editor link the bot sends for the scene's group now (US-B4). */
export async function currentLink(s: Scene): Promise<string> {
  await s.owner.send('/groups');
  return (await s.owner.message()).match(/\((http[^)]+)\)/)![1];
}

export function editorUrl(config: unknown, id: string | null = '7') {
  const link = settingsLink(id ?? '7', config).match(/\((http[^)]+)\)/)![1];
  return id === null ? link.replace(/bot_id=\d+&/, '') : link;
}

/** Adds a rule and makes its condition `type` (the picker's label); returns the rule pane. */
export async function addCondition(page: Page, type: string) {
  await page.getByRole('button', { name: '+ Add rule' }).click();
  const pane = page.getByRole('region', { name: 'Rule', exact: true });
  await pane.getByRole('combobox', { name: 'Condition type' }).first().selectOption({ label: type });
  return pane;
}
