const sidebarItems = ["Inbox shell", "Groups shell", "Communities shell"];

export default function App() {
  return (
    <main className="min-h-screen bg-matte-950 text-zinc-100">
      <div className="mx-auto flex min-h-screen w-full max-w-6xl flex-col gap-4 px-4 py-4 sm:px-6 lg:px-8">
        <header className="border-2 border-black bg-matte-900 p-4 shadow-brutal">
          <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
            <div>
              <p className="text-xs font-bold uppercase tracking-widest text-ember-300">
                Novastrum rebuild
              </p>
              <h1 className="mt-1 text-xl font-black text-zinc-50">
                Static app shell
              </h1>
            </div>
            <div className="border-2 border-black bg-matte-800 px-3 py-2 text-sm text-zinc-300">
              <span className="font-bold text-ember-400">Profile</span>
              <span className="ml-2">not connected</span>
            </div>
          </div>
        </header>

        <section className="grid flex-1 gap-4 lg:grid-cols-[260px_1fr]">
          <aside className="border-2 border-black bg-matte-900 p-4 shadow-brutal">
            <h2 className="text-sm font-black uppercase tracking-widest text-zinc-400">
              Context
            </h2>
            <nav className="mt-4 grid gap-2">
              {sidebarItems.map((item) => (
                <div
                  className="border-2 border-black bg-matte-800 px-3 py-3 text-sm font-bold text-zinc-200"
                  key={item}
                >
                  {item}
                </div>
              ))}
            </nav>
          </aside>

          <section className="border-2 border-black bg-zinc-100 p-4 text-matte-950 shadow-brutal sm:p-6">
            <div className="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div>
                <p className="text-xs font-black uppercase tracking-widest text-ember-500">
                  Main panel
                </p>
                <h2 className="mt-2 text-2xl font-black">
                  Ready for the first real product slice
                </h2>
              </div>
              <span className="border-2 border-black bg-ember-400 px-3 py-2 text-sm font-black uppercase text-black">
                Skeleton only
              </span>
            </div>

            <div className="mt-6 grid gap-3 sm:grid-cols-3">
              <StatusTile label="API" value="not wired" />
              <StatusTile label="Auth" value="not built" />
              <StatusTile label="WebSocket" value="not started" />
            </div>

            <p className="mt-6 max-w-2xl border-l-4 border-ember-500 pl-4 text-sm leading-6 text-zinc-700">
              Chat, REST integration, authentication, realtime packets, routing,
              and state management are intentionally not implemented in this
              skeleton.
            </p>
          </section>
        </section>
      </div>
    </main>
  );
}

function StatusTile({ label, value }: { label: string; value: string }) {
  return (
    <div className="border-2 border-black bg-white p-3">
      <p className="text-xs font-black uppercase tracking-widest text-zinc-500">
        {label}
      </p>
      <p className="mt-2 text-lg font-black text-matte-950">{value}</p>
    </div>
  );
}
