// The people's SimpleX CLI, driven through its websocket API: commands go in
// as the text one would type, responses and events come out as JSON.
//
// The CLI hands each event to one of its websocket clients only, whichever
// reads it first, so the people share a single connection, which passes every
// event to the person of the user it belongs to.
import WebSocket from 'ws';

export type Event = Record<string, any>;

class Queue {
  private items: Event[] = [];
  private waiting: ((e: Event | undefined) => void)[] = [];
  push(e: Event) {
    const w = this.waiting.shift();
    if (w) w(e);
    else this.items.push(e);
  }
  next(ms: number): Promise<Event | undefined> {
    const e = this.items.shift();
    if (e) return Promise.resolve(e);
    return new Promise((resolve) => {
      const done = (x: Event | undefined) => {
        clearTimeout(timer);
        resolve(x);
      };
      const timer = setTimeout(() => {
        this.waiting.splice(this.waiting.indexOf(done), 1);
        resolve(undefined);
      }, ms);
      this.waiting.push(done);
    });
  }
}

export class Cli {
  private corr = 0;
  private pending = new Map<string, (resp: Event) => void>();
  private queues = new Map<string, Queue>();

  private constructor(private ws: WebSocket) {
    ws.on('message', (data) => {
      let value: Event;
      try {
        value = JSON.parse(data.toString());
      } catch {
        return;
      }
      const respond = value.corrId && this.pending.get(value.corrId);
      if (respond) {
        this.pending.delete(value.corrId);
        respond(value.resp);
        return;
      }
      const user = value.resp?.user?.localDisplayName;
      if (user && this.queues.has(user)) this.queues.get(user)!.push(value.resp);
    });
  }

  static async connect(url: string): Promise<Cli> {
    const deadline = Date.now() + 60_000;
    for (;;) {
      try {
        const ws = await new Promise<WebSocket>((resolve, reject) => {
          const ws = new WebSocket(url);
          ws.once('open', () => resolve(ws));
          ws.once('error', reject);
        });
        return new Cli(ws);
      } catch (err) {
        if (Date.now() > deadline) throw new Error(`cannot connect to the SimpleX CLI at ${url}: ${err}`);
        await new Promise((r) => setTimeout(r, 1000));
      }
    }
  }

  /** Sends `cmd` and returns its response. */
  command(cmd: string): Promise<Event> {
    const corrId = String(++this.corr);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`no response to ${cmd} in 60s`)), 60_000);
      this.pending.set(corrId, (resp) => {
        clearTimeout(timer);
        resolve(resp);
      });
      this.ws.send(JSON.stringify({ corrId, cmd }));
    });
  }

  /** The events of the user `name`, from now on. */
  listen(name: string): Queue {
    const q = new Queue();
    this.queues.set(name, q);
    return q;
  }

  close() {
    this.ws.close();
  }
}

export type { Queue };
