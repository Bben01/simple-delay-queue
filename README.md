# A simple async Delay Queue

A lightweight and efficient time-based queue implementation for Tokio that delays yielding inserted elements until a fixed timeout has elapsed.

[![Crates.io](https://img.shields.io/crates/v/simple-delay-queue.svg)](https://crates.io/crates/time_queue)
[![Documentation](https://docs.rs/time_queue/badge.svg)](https://docs.rs/time_queue)

## Overview

`TimeQueue` provides a simple, performant queue that yields elements in FIFO order after a fixed timeout has elapsed. Unlike more complex delay queues, `TimeQueue` focuses on efficiency with O(1) push and pop operations by enforcing a constant timeout for all elements.

## Features

- **Fixed Timeout**: Uses a constant timeout for all elements, optimizing for simplicity and performance
- **FIFO Order**: Elements expire in the exact order they were inserted
- **O(1) Operations**: Both push and pop operations are constant time
- **Tokio Integration**: Seamlessly works with Tokio's async runtime
- **Stream Implementation**: Can be used with Rust's async stream combinators

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
time_queue = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

## Usage

```rust
use std::time::Duration;
use simple_delay_queue::TimeQueue;
use futures_util::StreamExt;

#[tokio::main]
async fn main() {
    // Create a TimeQueue with a 500ms timeout
    let mut queue = TimeQueue::new(Duration::from_millis(500));
    
    // Add elements to the queue
    queue.push("first");
    queue.push("second");
    queue.push("third");
    
    // Elements will become available after their timeout
    while let Some(item) = queue.next().await {
        println!("Got item: {}", item);
    }
}
```

## When to Use TimeQueue

`TimeQueue` is ideal for:

- Rate limiting
- Implementing simple delays
- Processing items in order after a fixed cooldown period
- Any scenario where all items should be delayed by the same amount of time

## Comparison with `tokio::time::DelayQueue`

`TimeQueue` is designed to be simpler and faster than `tokio::time::DelayQueue` for specific use cases:

| Feature               | TimeQueue                     | DelayQueue                  |
|-----------------------|-------------------------------|-----------------------------|
| Timeout Configuration | Fixed for all elements        | Configurable per element    |
| Order Guarantee       | FIFO                          | Based on expiration time    |
| Performance           | O(1) push/pop                 | O(log n)                    |
| Reset Timeouts        | No                            | Yes                         |
| Remove Elements       | No                            | Yes                         |
| Use Case              | Simple, efficient delay queue | Feature-rich priority queue |

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Serde by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
</sub>
