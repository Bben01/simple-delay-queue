use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use futures_util::StreamExt;
use simple_delay_queue::TimeQueue;
use std::time::Duration;
use tokio_util::time::DelayQueue;

fn bench_push_pop(c: &mut Criterion) {
    let mut group = c.benchmark_group("queue_push_pop");

    for size in [1, 10, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("TimeQueue", size), size, |b, &size| {
            b.to_async(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap(),
            )
            .iter(|| async {
                let mut queue = TimeQueue::with_capacity(Duration::from_secs(0), size);

                // Push elements
                for i in 0..size {
                    queue.push(i);
                }

                // Pop all elements
                use futures_util::StreamExt;
                while let Some(_) = queue.next().await {
                    if queue.is_empty() {
                        break;
                    }
                }
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

                // Push elements
                for i in 0..size {
                    queue.insert(i, Duration::from_millis(0));
                }

                // Pop all elements
                while let Some(_) = queue.next().await {
                    if queue.is_empty() {
                        break;
                    }
                }
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_push_pop,
);
criterion_main!(benches);
