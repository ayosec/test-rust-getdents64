# Benchmarks for `test-getdents64`

The scripts in this directory can be used to launch multiple benchmarks against
the program implemented at the root of the repository.

## Process

The script `benchs/run` executes the following process:

1. Populate a directory to have a specific number of files.

    The number of files is controlled by the `FILE_COUNTS` constant.

2. Execute the main program using the standard library version, and the
   experimental implementation using `getdents64` with no libc wrappers.

3. Writes a JSON object with the stats emitted from the main program.

## How to use

1. Build the main program.

    ```console
    $ cargo build --release
    ```

2. Execute the `benchs/run` script, and redirects its standard output to a file.

    ```console
    $ ruby benchs/run > target/results.json
    ```

    The script creates a temporary directory to generate files for the
    benchmark. An optional argument in the command-line can be used to control
    the path of this directory.

3. When the script is finished, render the results as Markdown.

    ```console
    $ ruby benchs/render-results target/results.json > results.md
    ```
