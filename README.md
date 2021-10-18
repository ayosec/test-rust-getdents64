# test-rust-getdents64

This repository contains an experimental implementation of [`std::fs::ReadDir`]
using the `getdents64` syscall directly, without the libc wrappers.

More information in [the thread in `internals.r.o`].

See [`benchs/README`] for information about how to run the benchmark.

[`benchs/README`]: ./benchs/README.md
[`std::fs::ReadDir`]: https://doc.rust-lang.org/stable/std/fs/struct.ReadDir.html
[the thread in `internals.r.o`]: https://internals.rust-lang.org/t/fs-read-dir-cant-read-directory-entries-with-very-long-names/15447
