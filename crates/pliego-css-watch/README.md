# pliego-css-watch

Exact-version native filesystem-event wakeups for `pliego-cssc watch`.

Events only schedule authoritative full snapshots. Windows uses change-notification handles and
Linux uses recursive `inotify` registration. The small Windows backend is the workspace's isolated
native/unsafe boundary. Unsupported backends fail explicitly so the CLI retains
correctness-preserving 100 ms polling.
