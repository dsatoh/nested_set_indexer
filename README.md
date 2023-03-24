# Nested set indexer

[![GitHub](https://img.shields.io/github/license/dsatoh/nested_set_indexer)](https://github.com/dsatoh/nested_set_indexer/blob/main/LICENSE)
[![.github/workflows/release.yml](https://github.com/dsatoh/nested_set_indexer/actions/workflows/release.yml/badge.svg)](https://github.com/dsatoh/nested_set_indexer/actions/workflows/release.yml)
[![GitHub release (latest SemVer)](https://img.shields.io/github/v/release/dsatoh/nested_set_indexer?logo=github)](https://github.com/dsatoh/nested_set_indexer/releases)

A command-line tool for assigning left/right indices of nested set

## Usage

* From standard input

  ```shell
  $ nested_set_indexer < input.json > output.json 
  ```

  ```
  nested_set_indexer [OPTIONS] [input]

  FLAGS:
      -h, --help       Prints help information
      -V, --version    Prints version information

  OPTIONS:
      -f, --from <from>        Input format [possible values: csv, tsv, json]
      -o, --output <output>    Output to a file (default: stdout)
      -t, --to <to>            Output format [possible values: csv, tsv, json]

  ARGS:
      <input>    File to process (default: stdin)
  ```
