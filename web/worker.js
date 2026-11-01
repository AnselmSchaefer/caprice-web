// A Web Worker for one piece of Caprice's background work (src/platform/worker.rs): it starts the
// app's WebAssembly on the memory it shares with the page, runs the work, says so, and closes.
self.onmessage = async ({ data: [script, module, memory, work] }) => {
  const app = await import(script);
  await app.default({ module_or_path: module, memory });
  app.caprice_worker_run(work);
  postMessage("done");
  close();
};
