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
Details:
```text
-k -> Filter by keys. Input ckey(s) that you wish to filter logs by.
Unlike regular searching, this ONLY takes the logs FROM the ckeys.
```
```text
-p -> Coordinate based filtering, shows logs within a certain range of
the input coordinates.
```
```text
-P -> Takes two ckeys and displays logs where both are within proximity
of each other (based on logged coordinates). Finicky, but has uses.
```
```text
-f -> Takes a ckey and shows all logs LOCAL to their position. It is
effectively a moving window filter, but has an a flaw in that it will
only move the window when the chosen ckey is logged at a new spot.
```
```text
-r -> Changes the range of coordinate based filters from the default.
```

## Examples (./ for linux, .\ for windows)

Taking in game.log and attack.log:
```text
./logfilter -i game.log attack.log
```

Changing name of output file:
```text
./logfilter -i game.log attack.log -o output.log
```

Filtering for ckey "Glowpoke":
```text
./logfilter -i game.log -k Glowpoke
```

Filtering for all logs within (non-default) 10 tiles in both X&Y, on the same Z level,
of location (100, 150, 3), as well as only showing ckey "Glowpoke"'s logs:
```text
./logfilter -i game.log -p 100,150,3 -r 10,10,0 -k Glowpoke
```

Filtering for all logs where ckeys "Glowpoke" and "Pokeglow" are within proximity of each other:
```text
./logfilter -i game.log -P Glowpoke Pokeglow
```

Filtering for logs around ckey "Glowpoke":
```text
./logfilter -i game.log -f Glowpoke
```

[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](https://www.gnu.org/licenses/gpl-3.0)