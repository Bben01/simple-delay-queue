//! A crate providing a `TimeQueue` that delays yielding inserted elements until a fixed timeout
//! has elapsed. Each inserted element is stored together with its expiration time (current Tokio
//! `Instant` plus the constant timeout). Since the timeout is constant, the elements naturally
//! expire in FIFO order, and both push and pop operations are O(1).
//!
//! # Differences with `tokio::time::DelayQueue`
//!
//! The `TimeQueue` in this crate is designed to be simpler and faster than `tokio::time::DelayQueue`.
//! While `DelayQueue` offers more features such as the ability to reset timeouts and remove elements
//! before they expire, `TimeQueue` focuses on providing a minimalistic and efficient implementation
//! for cases where these additional features are not needed.
//!
//! Key differences:
//! - **Fixed Timeout**: `TimeQueue` uses a constant timeout for all elements, whereas `DelayQueue`
//!   allows specifying different timeouts for each element.
//! - **FIFO Order**: Elements in `TimeQueue` expire in the order they were inserted, ensuring FIFO
//!   order. `DelayQueue` does not guarantee FIFO order if elements have different timeouts.
//! - **Performance**: `TimeQueue` is optimized for performance with O(1) push and pop operations,
//!   making it faster for use cases where the additional features of `DelayQueue` are not required.
use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
    time::Duration,
};

use futures_core::Stream;
use tokio::time::{sleep_until, Instant, Sleep};

/// A time queue that delays yielding inserted elements until a fixed timeout
/// has elapsed. Each inserted element is stored together with its expiration
/// time (current Tokio Instant plus the constant timeout). Since the timeout
/// is constant, the elements naturally expire in FIFO order, and both push
/// and pop operations are O(1).
pub struct TimeQueue<T> {
    /// Constant timeout duration for every element.
    timeout: Duration,
    /// FIFO queue storing elements paired with their expiration time.
    queue: VecDeque<(Instant, T)>,
    /// The currently active timer to wake up the task when the next element expires.
    /// The sleep future is stored pinned manually.
    timer: Option<Pin<Box<Sleep>>>,
}

impl<T> TimeQueue<T> {
    /// Creates a new `TimeQueue` with the given timeout.
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            queue: VecDeque::new(),
            timer: None,
        }
    }

    /// Creates a new `TimeQueue` with the given timeout and reserves capacity for the underlying queue.
    pub fn with_capacity(timeout: Duration, capacity: usize) -> Self {
        Self {
            timeout,
            queue: VecDeque::with_capacity(capacity),
            timer: None,
        }
    }

    /// Inserts an element into the queue. The element will be yielded after
    /// `timeout` has elapsed from the time of insertion.
    pub fn push(&mut self, element: T) {
        // Compute the expiration time based on Tokio's clock.
        let expire_time = Instant::now() + self.timeout;
        self.queue.push_back((expire_time, element));

        // If there is no timer active, set one.
        if self.timer.is_none() {
            self.set_timer();
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Sets (or resets) the timer to fire at the expiration time of the element
    /// at the front of the queue.
    fn set_timer(&mut self) {
        if let Some(&(instant, _)) = self.queue.front() {
            // Create a new sleep future and pin it manually.
            self.timer = Some(Box::pin(sleep_until(instant)));
        }
    }
}

impl<T> Stream for TimeQueue<T> {
    type Item = T;

    /// Polls the stream to yield the next element whose timeout has expired.
    ///
    /// If no element is ready, this method registers the current task to be
    /// woken when the next element's timeout is reached.
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // If the queue is empty, clear any timer and return Pending.
        if self.queue.is_empty() {
            self.timer = None;
            return Poll::Pending;
        }

        // Ensure there is an active timer.
        if self.timer.is_none() {
            self.set_timer();
        }

        // Now, we have a timer. It is stored as a Pin<Box<Sleep>>,
        // so we can poll it using its pinned projection.
        let timer = self.timer.as_mut().expect("timer should be set");
        if timer.as_mut().poll(cx).is_pending() {
            return Poll::Pending;
        }

        // Timer has fired; remove the expired element from the front.
        let (_instant, element) = self.queue.pop_front().expect("queue was non-empty");

        // Reset the timer for the next element if any remain.
        if !self.queue.is_empty() {
            self.set_timer();
        } else {
            self.timer = None;
        }

        Poll::Ready(Some(element))
    }
}

// It is safe to implement Unpin manually because our type does not use any self-referential patterns.
impl<T> Unpin for TimeQueue<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;
    use std::time::Duration;
    use tokio::time::advance;

    /// Test that polling an empty queue remains pending.
    #[tokio::test(start_paused = true)]
    async fn test_empty_queue() {
        let timeout = Duration::from_secs(600);
        let mut queue: TimeQueue<u64> = TimeQueue::new(timeout);
        // Without any insertions, polling should never yield an element.
        tokio::select! {
            biased;
            _ = queue.next() => panic!("Queue should be empty and pending"),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }
    }

    /// Test that an element is not returned before its timeout expires.
    #[tokio::test(start_paused = true)]
    async fn test_element_not_ready_immediately() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);
        queue.push(42);
        // Immediately polling should not return the element.
        tokio::select! {
            biased;
            _ = queue.next() => panic!("Element should not be ready immediately"),
            _ = tokio::time::sleep(Duration::from_millis(50)) => {},
        }
    }

    /// Test that multiple elements inserted are yielded in FIFO order
    /// once their individual timeouts have elapsed.
    #[tokio::test(start_paused = true)]
    async fn test_time_queue_order_with_insertion_gap() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);
        // Insert the first element.
        queue.push(1);
        // Advance time by half the timeout, then insert another.
        advance(timeout / 2).await;
        queue.push(2);
        // Advance time so that the first element expires but the second isn't ready yet.
        advance(timeout / 2).await;
        // Now the first element should be ready.
        assert_eq!(queue.next().await, Some(1));
        // The second element should not be ready yet.
        tokio::select! {
            biased;
            _ = queue.next() => panic!("Second element should not be ready yet"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        // Advance time so that the second element expires.
        advance(timeout / 2).await;
        assert_eq!(queue.next().await, Some(2));
    }

    /// Test that repeated polls before an element is ready remain pending.
    #[tokio::test(start_paused = true)]
    async fn test_repeated_polling() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);
        queue.push(100);
        // Poll several times within the timeout period.
        for _ in 0..5 {
            tokio::select! {
                biased;
                _ = queue.next() => panic!("Element should not be ready yet"),
                _ = tokio::time::sleep(Duration::from_millis(10)) => {},
            }
        }
        // Advance time and ensure the element is then ready.
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(100));
    }

    /// Test that inserting an element after a previous one has expired behaves correctly.
    #[tokio::test(start_paused = true)]
    async fn test_insert_after_timeout() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);
        queue.push(100);
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(100));

        // Insert another element after the first has expired.
        queue.push(200);
        // Immediately after insertion, it should not be ready.
        tokio::select! {
            biased;
            _ = queue.next() => panic!("Element should not be ready immediately"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        // Advance time so that the new element expires.
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(200));
    }

    /// Test interleaved insertion and polling to ensure that the timer
    /// is correctly re-armed and elements are yielded in the proper order.
    #[tokio::test(start_paused = true)]
    async fn test_interleaved_inserts() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);

        // Insert a couple of elements.
        queue.push(10);
        queue.push(20);

        // Advance time just past the timeout of the first element.
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(10));

        // Immediately insert a new element.
        queue.push(30);

        // The second element (20) should be ready since its timeout expired.
        assert_eq!(queue.next().await, Some(20));

        // The newly inserted element should not be ready until its own timeout.
        tokio::select! {
            biased;
            _ = queue.next() => panic!("Newly inserted element should not be ready immediately"),
            _ = tokio::time::sleep(Duration::from_millis(10)) => {},
        }
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(30));
    }

    /// Test that `poll_next` is cancellation safe.
    #[tokio::test(start_paused = true)]
    async fn test_poll_next_cancellation_safety() {
        let timeout = Duration::from_secs(600);
        let mut queue = TimeQueue::new(timeout);
        queue.push(42);

        // Poll the queue once, but do not await the result.
        let mut poll_future = Box::pin(queue.next());
        let waker = futures_util::task::noop_waker();
        let mut cx = Context::from_waker(&waker);

        // Poll the future to ensure it registers the waker.
        assert!(poll_future.as_mut().poll(&mut cx).is_pending());

        // Drop the future to simulate cancellation.
        drop(poll_future);

        // Ensure the queue is still in a consistent state.
        assert_eq!(queue.len(), 1);

        // Advance time and ensure the element is then ready.
        advance(timeout).await;
        assert_eq!(queue.next().await, Some(42));
    }
}
