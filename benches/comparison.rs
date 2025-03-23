use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use futures_util::StreamExt;
use simple_delay_queue::TimeQueue;
use std::time::Duration;
use tokio_util::time::DelayQueue;

fn bench_push_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_push_pop");

    for size in [1000].iter() {
        group.bench_with_input(BenchmarkId::new("TimeQueue", size), size, |b, &size| {
            b.to_async(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
            )
            .iter(|| async {
                let mut queue = TimeQueue::with_capacity(Duration::from_millis(1), size);

                // Push elements
                for i in 0..size {
                    queue.push(i);
                }

                // Advance time to expire all elements
                tokio::time::pause();
                tokio::time::advance(Duration::from_millis(2)).await;

                // Pop all elements
                let mut count = 0;
                use futures_util::StreamExt;
                while let Some(_) = queue.next().await {
                    count += 1;
                    if count == size {
                        break;
                    }
                }

                tokio::time::resume();
            });
        });

        group.bench_with_input(BenchmarkId::new("DelayQueue", size), size, |b, &size| {
            b.to_async(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
            )
            .iter(|| async {
                let mut queue = DelayQueue::with_capacity(size);
                let mut keys = Vec::with_capacity(size);

                // Push elements
                for i in 0..size {
                    let key = queue.insert(i, Duration::from_millis(1));
                    keys.push(key);
                }

                // Advance time to expire all elements
                tokio::time::pause();
                tokio::time::advance(Duration::from_millis(2)).await;

                // Pop all elements
                let mut count = 0;
                while let Some(_) = queue.next().await {
                    count += 1;
                    if count == size {
                        break;
                    }
                }

                tokio::time::resume();
            });
        });
    }

    group.finish();
}

fn bench_push(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_push");

    for size in [10, 100, 1000, 10000].iter() {
        group.bench_with_input(BenchmarkId::new("TimeQueue", size), size, |b, &size| {
            b.iter(|| {
                let mut queue = TimeQueue::with_capacity(Duration::from_millis(1), size);
                for i in 0..size {
                    queue.push(i);
                }
            });
        });

        group.bench_with_input(BenchmarkId::new("DelayQueue", size), size, |b, &size| {
            b.iter(|| {
                let mut queue = DelayQueue::with_capacity(size);
                for i in 0..size {
                    queue.insert(i, Duration::from_millis(1));
                }
            });
        });
    }

    group.finish();
}

fn bench_variable_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("variable_load");

    // Different patterns of pushes and pops
    // 1. Push heavy: 80% pushes, 20% pops
    // 2. Pop heavy: 20% pushes, 80% pops
    // 3. Balanced: 50% pushes, 50% pops

    for (pattern_name, push_ratio) in [("push_heavy", 0.8), ("balanced", 0.5), ("pop_heavy", 0.2)] {
        let operations = 1000;

        group.bench_with_input(
            BenchmarkId::new("TimeQueue", pattern_name),
            &(operations, push_ratio),
            |b, &(ops, ratio)| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let mut queue = TimeQueue::new(Duration::from_millis(1));
                        let mut count = 0;

                        tokio::time::pause();

                        for _ in 0..ops {
                            if rand::random::<f64>() < ratio {
                                // Push operation
                                queue.push(count);
                                count += 1;
                            } else {
                                // Pop operation
                                tokio::time::advance(Duration::from_millis(2)).await;
                                use futures_util::StreamExt;
                                let _ = queue.next().await;
                            }

                            // Periodically advance time
                            if count % 10 == 0 {
                                tokio::time::advance(Duration::from_millis(1)).await;
                            }
                        }

                        tokio::time::resume();
                    });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("DelayQueue", pattern_name),
            &(operations, push_ratio),
            |b, &(ops, ratio)| {
                b.to_async(tokio::runtime::Runtime::new().unwrap())
                    .iter(|| async {
                        let mut queue = DelayQueue::new();
                        let mut count = 0;
                        let mut keys = Vec::new();

                        tokio::time::pause();

                        for _ in 0..ops {
                            if rand::random::<f64>() < ratio {
                                // Push operation
                                let key = queue.insert(count, Duration::from_millis(1));
                                keys.push(key);
                                count += 1;
                            } else if !keys.is_empty() {
                                // Pop operation
                                tokio::time::advance(Duration::from_millis(2)).await;
                                let _ = queue.next().await;
                            }

                            // Periodically advance time
                            if count % 10 == 0 {
                                tokio::time::advance(Duration::from_millis(1)).await;
                            }
                        }

                        tokio::time::resume();
                    });
            },
        );
    }

    group.finish();
}

fn bench_large_queue_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("large_queue_lookup");

    for size in [1000, 10000, 100000].iter() {
        group.bench_with_input(BenchmarkId::new("TimeQueue", size), size, |b, &size| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let mut queue = TimeQueue::with_capacity(Duration::from_millis(100), size);

                    // Fill queue with elements
                    for i in 0..size {
                        queue.push(i);
                    }

                    // Advance time partially - only expire half the elements
                    tokio::time::pause();
                    tokio::time::advance(Duration::from_millis(50)).await;

                    // Pop half the elements
                    use futures_util::StreamExt;
                    for _ in 0..(size / 2) {
                        if queue.next().await.is_none() {
                            break;
                        }
                    }

                    tokio::time::resume();
                });
        });

        group.bench_with_input(BenchmarkId::new("DelayQueue", size), size, |b, &size| {
            b.to_async(tokio::runtime::Runtime::new().unwrap())
                .iter(|| async {
                    let mut queue = DelayQueue::with_capacity(size);

                    // Fill queue with elements
                    for i in 0..size {
                        queue.insert(i, Duration::from_millis(100));
                    }

                    // Advance time partially - only expire half the elements
                    tokio::time::pause();
                    tokio::time::advance(Duration::from_millis(50)).await;

                    // Pop half the elements
                    for _ in 0..(size / 2) {
                        if queue.next().await.is_none() {
                            break;
                        }
                    }

                    tokio::time::resume();
                });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_push_pop,
    // bench_push,
    // bench_variable_load,
    // bench_large_queue_lookup
);
criterion_main!(benches);
