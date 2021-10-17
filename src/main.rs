//! Experimental implementation for `std::fs::read_dir` using `getdents64`.

use std::env;
use std::io::{self, Write};
use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, Instant};

mod fs;

/// Number of iterations before to start to compute time duration.
const BENCH_WARMUP: usize = 5;

/// Duration of the benchmark for every directory.
const DEFAULT_BENCH_DURATION: Duration = Duration::from_secs(3);

const USAGE: &str = "\
Usage: test-getdents64 [options] dirs..

Options:

    -v          Print duration after every run.
    -p          Print file names.
    -s          Use `std::fs::read_dir`.
    -d secs     Set the benchmark duration (seconds).
";

fn main() {
    let mut verbose = false;
    let mut print_names = false;
    let mut use_std = false;
    let mut bench_duration = DEFAULT_BENCH_DURATION;

    check_traits::<fs::ReadDir>();

    let mut args = env::args_os().skip(1);
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-v") => verbose = true,

            Some("-p") => print_names = true,

            Some("-s") => use_std = true,

            Some("-d") => {
                bench_duration = args
                    .next()
                    .as_ref()
                    .and_then(|a| a.to_str())
                    .and_then(|a| str::parse(a).ok())
                    .map(Duration::from_secs)
                    .expect("Expected integer argument.");
            }

            Some("-h" | "--help") => {
                eprint!("{}", USAGE);
                return;
            }

            _ => {
                macro_rules! read_dir {
                    ($iter:expr) => {{
                        let stdout_handle = io::stdout();
                        let mut stdout = io::BufWriter::new(stdout_handle.lock());

                        let mut duration_sum = Duration::default();
                        let mut duration_max = Duration::default();
                        let mut duration_min = Duration::from_secs(u64::MAX);
                        let mut iter_count = 0;

                        let main_loop_start = Instant::now();
                        while main_loop_start.elapsed() < bench_duration {
                            let run_start = Instant::now();
                            for f in $iter.unwrap() {
                                if print_names {
                                    let path = f.unwrap().path();

                                    stdout
                                        .write_all(path.file_name().unwrap().as_bytes())
                                        .unwrap();

                                    stdout.write_all(b"\n").unwrap();
                                }
                            }

                            iter_count += 1;
                            if iter_count > BENCH_WARMUP {
                                let elapsed = run_start.elapsed();

                                duration_sum += elapsed;
                                duration_min = duration_min.min(elapsed);
                                duration_max = duration_max.max(elapsed);

                                if verbose {
                                    eprintln!("{:?}", elapsed);
                                }
                            }

                            // Only one run if file names are written to stdout.
                            if print_names {
                                break;
                            }
                        }

                        if iter_count > BENCH_WARMUP {
                            eprintln!(
                                "AVG: {} ms | MAX: {} ms | MIN: {} ms",
                                duration_sum.as_secs_f64() / (iter_count - BENCH_WARMUP) as f64
                                    * 1000.0,
                                duration_max.as_secs_f64() * 1000.0,
                                duration_min.as_secs_f64() * 1000.0,
                            );
                        }
                    }};
                }

                if use_std {
                    read_dir!(std::fs::read_dir(&arg));
                } else {
                    read_dir!(fs::read_dir(&arg));
                }
            }
        }
    }
}

fn check_traits<T: Sync + Send>() {}
