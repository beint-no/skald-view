/** One running render and one replaceable pending render: queue memory is bounded. */
export class LatestQueue {
  active = false;
  pending = null;
  closed = false;
  constructor(run) {
    this.run = run;
  }
  submit(value) {
    if (this.closed) return Promise.reject(new Error("Queue closed"));
    return new Promise((resolve, reject) => {
      if (this.pending)
        this.pending.reject(new DOMException("Superseded", "AbortError"));
      this.pending = { value, resolve, reject };
      this.pump();
    });
  }
  async pump() {
    if (this.active || !this.pending || this.closed) return;
    this.active = true;
    const job = this.pending;
    this.pending = null;
    try {
      job.resolve(await this.run(job.value));
    } catch (e) {
      job.reject(e);
    } finally {
      this.active = false;
      this.pump();
    }
  }
  close() {
    this.closed = true;
    this.pending?.reject(new DOMException("Closed", "AbortError"));
    this.pending = null;
  }
}
