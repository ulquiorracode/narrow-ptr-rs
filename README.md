# narrow-ptr-rs

[![Crates.io](https://img.shields.io/crates/v/narrow-ptr.svg)](https://crates.io/crates/narrow-ptr)
[![Documentation](https://docs.rs/narrow-ptr/badge.svg)](https://docs.rs/narrow-ptr)
[![CI](https://github.com/ulquiorracode/narrow-ptr-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ulquiorracode/narrow-ptr-rs/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

Position-independent relative and narrow pointer types for zero-copy memory mapping (`mmap`), IPC, and memory-efficient data structures.

Conforms to the **"Scrooge" Systems Mindset** (zero memory waste, mechanical sympathy).

---

## Features

- **Position Independence**: Unlike absolute 64-bit pointers (`*const T`, `&T`), `RelPtr<T, Offset>` stores relative displacement from its own memory address. Memory maps can be dumped to disk, sent over network ring buffers (`wire-ring`), or shared via IPC without pointer swizzling.
- **Pointer Compression (50–75% Memory Reduction)**:
  - `RelPtr<T, i32>`: 4 bytes (addresses displacements up to $\pm 2$ GiB)
  - `RelPtr<T, i16>`: 2 bytes (addresses displacements up to $\pm 32$ KiB)
- **Context-Bounded Safety (`resolve_within`)**: Safe dereferencing with boundary verification against arena slices.
- **`#![no_std]` by Default**: Usable in embedded firmware, OS kernels, and WASM sandbox runtimes.

---

## Quick Start

```rust
use narrow_ptr::RelPtr;

#[repr(C)]
struct Node {
    value: u32,
    next: RelPtr<Node, i32>,
}

let mut first = Node { value: 1, next: RelPtr::from_raw_offset(0) };
let second = Node { value: 2, next: RelPtr::from_raw_offset(0) };

// Wire relative pointer
first.next = RelPtr::new(&first.next, &second);

// Dereference
unsafe {
    assert_eq!((*first.next.as_ptr()).value, 2);
}
```

---

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT) at your option.
