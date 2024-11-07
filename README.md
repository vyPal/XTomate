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

[tasks.task2]
command = "echo World"
dependencies = ["task1"] # Task dependencies

[tasks.task3]
command = "echo Hello World"
dependencies = [{"task1" = "success"}, {"task2" = "success"}] # Task dependencies with status
```
