# XTomate
Your poweful automation ally

## Workflow TOML syntax
(Partially so that I don't forget)

```toml
name = "My workflow"
version = "0.1.0"
description = "My workflow description"

[tasks.task1] # Task name
command = "echo Hello" # Command to execute

[task2] # Another way to define a task
command = "echo World"
dependencies = ["task1" = "success"] # Dependencies
```
