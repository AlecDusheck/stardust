# Stardust
Stardust implementation in Rust.

## Code style
Comments should be concise; if you find yourself having a paragraph to explain something, that something is probably bad code, re-work it. No long comments. They should be concise and explain weird behavior/math, qwirks, etc.

Write idiomatic and portable Rust. Should run on Mac/Windows/Web in wasm

## Levels
Design a very good format to store levels in. Write an adapater so we can port levels from the original game easily. Make a level editor too.

## Original game
My copy is in `./archive`. This is my copy

### Licenses
Abandonware. The original author has said modifications are OK (you'll see comments in the games code talking about this). Write scripts that will extract the art and levels and put in the `vendor` folder. Write these scripts in Rust as well as a CLI
