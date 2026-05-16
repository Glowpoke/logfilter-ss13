# logfilter

This is just a fairly simple filter for ss13 logs made with rust.
Currently only made for AzurePeak logs, I have no idea if it works for other ones.

## Installation

It comes with a single file. You just run it in a terminal.


## Usage

Linux:
```text
./logfilter -i <file1> [file2 ...] [-o <output>] [OPTIONS]
```

Windows:
```text
.\logfilter -i <file1> [file2 ...] [-o <output>] [OPTIONS]
```

Options:
```text
  -k, --key key1,key2,...
  -p, --point x,y,z
  -r, --range dx,dy,dz (default 15,15,0)
  -P, --proximity keyA keyB
  -f, --follow key

Note: -p, -P, and -f are mutually exclusive.
```


[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)