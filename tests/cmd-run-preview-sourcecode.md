# `run` command - preview source code

Use `--preview-sourcecode` to show source code that is copied for an
operation.

    >>> use_example("sourcecode")

    >>> run("gage run default --preview-sourcecode", env={"COLUMNS": "50"})  #+diff
    Source Code
    |                                                |
    | | patterns                             |       |
    | |--------------------------------------|       |
    | | **/* text size<10000 max-matches=500 |       |
    | | -**/.* dir                           |       |
    | | -**/* dir sentinel=bin/activate      |       |
    | | -**/* dir sentinel=.nocopy           |       |
    |                                                |
    |                                                |
    | | matched files  |                             |
    | |----------------|                             |
    | | data.txt       |                             |
    | | gage.toml      |                             |
    | | hello_world.py |                             |
    |                                                |
    <0>

    >>> run("gage run default --preview-sourcecode --json")  # +parse
    {
      "sourcecode": {
        "src_dir": "{:path}",
        "patterns": [
          "**/* text size<10000 max-matches=500",
          "-**/.* dir",
          "-**/* dir sentinel=bin/activate",
          "-**/* dir sentinel=.nocopy"
        ],
        "paths": [
          "data.txt",
          "gage.toml",
          "hello_world.py"
        ]
      }
    }
    <0>

    >>> run("gage run pyfiles --preview-sourcecode --json")  # +parse
    {
      "sourcecode": {
        "src_dir": "{:path}",
        "patterns": [
          "*.py"
        ],
        "paths": [
          "hello_world.py"
        ]
      }
    }
    <0>

    >>> run("gage run exclude-data --preview-sourcecode --json")  # +parse
    {
      "sourcecode": {
        "src_dir": "{:path}",
        "patterns": [
          "**/* text size<10000 max-matches=500",
          "-**/.* dir",
          "-**/* dir sentinel=bin/activate",
          "-**/* dir sentinel=.nocopy",
          "-data.txt"
        ],
        "paths": [
          "gage.toml",
          "hello_world.py"
        ]
      }
    }
    <0>

    >>> run("gage run all-files --preview-sourcecode --json")  # +parse
    {
      "sourcecode": {
        "src_dir": "{:path}",
        "patterns": [
          "**/*"
        ],
        "paths": [
          "data.txt",
          "gage.toml",
          "hello_world.py",
          "hello_world.pyc"
        ]
      }
    }
    <0>
