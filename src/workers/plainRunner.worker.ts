import { runPlain } from '../lib/plainRuntime';

const worker = self as unknown as {
  addEventListener: (type: 'message', listener: (event: MessageEvent<{ source: string }>) => void) => void;
  postMessage: (message: unknown) => void;
};
worker.addEventListener('message', (event) => {
  worker.postMessage(runPlain(event.data.source));
});
