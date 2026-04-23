// Vitest setup: ensure the React development bundle is loaded so `act(...)`
// works under jsdom. The define() in vitest.config.ts primes NODE_ENV, and this
// flag tells React Testing Library this environment supports act.
(globalThis as unknown as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
