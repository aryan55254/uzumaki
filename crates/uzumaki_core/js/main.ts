import { dispatchEvent } from './react/reconciler';
import { listenAppEvents } from './bindings';

console.log('worker started');
const entryPoint = process.env.entryPoint;

if (!entryPoint) {
  throw new Error('entryPoint not set');
}

// Register DOM event listener via ThreadsafeFunction so Rust can call us directly
listenAppEvents((err, event) => {
  if (err) {
    console.error('DOM event error:', err);
    return;
  }
  dispatchEvent(event.nodeId, event.eventType);
});

try {
  await import(entryPoint);
} catch (e) {
  console.error('Error running entry point');
  console.error(e);
  process.exit(1);
}
