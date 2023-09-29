export class Timer {
  fn: () => void;
  interval: number;
  timerHandle: number | null;

  constructor(interval: number, target?: () => void) {
    this.interval = interval;
    this.fn = target || (() => {});
    this.timerHandle = null;
  }

  reset = () => {
    if (this.timerHandle !== null) {
      clearInterval(this.timerHandle);
      this.timerHandle = null;
    }
  };

  // run() will reset the timer and then run the target function
  // after the interval, assuming it isn't reset again.
  run = (fn?: () => void) => {
    this.reset();
    if (fn) {
      this.fn = fn;
    }
    this.timerHandle = setTimeout(this.fn, this.interval);
  };
}
