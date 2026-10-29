use std::{future::Future, time::Duration};

use futures_util::{Stream, StreamExt};

#[derive(Clone, Copy)]
pub struct FirstTokenDeadline {
    deadline: tokio::time::Instant,
}

#[derive(Debug, Clone, Copy)]
pub struct FirstTokenTimeout;

impl FirstTokenDeadline {
    pub fn from_start(started: std::time::Instant, timeout: Duration) -> Self {
        Self {
            deadline: tokio::time::Instant::from_std(started + timeout),
        }
    }

    pub async fn run<F: Future>(&self, future: F) -> Result<F::Output, FirstTokenTimeout> {
        if tokio::time::Instant::now() >= self.deadline {
            return Err(FirstTokenTimeout);
        }
        tokio::time::timeout_at(self.deadline, future)
            .await
            .map_err(|_| FirstTokenTimeout)
    }

    pub async fn next<S: Stream + Unpin>(
        &self,
        stream: &mut S,
    ) -> Result<Option<S::Item>, FirstTokenTimeout> {
        self.run(stream.next()).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Bytes;

    #[tokio::test]
    async fn expired_deadline_times_out_even_for_an_ready_future() {
        let started = std::time::Instant::now() - Duration::from_secs(2);
        let deadline = FirstTokenDeadline::from_start(started, Duration::from_secs(1));

        assert!(deadline.run(async {}).await.is_err());
    }

    #[tokio::test]
    async fn reads_stream_items_under_the_same_absolute_deadline() {
        let expected = Bytes::from_static(b"first token");
        let mut stream =
            futures_util::stream::iter([Ok::<_, &'static str>(Bytes::new()), Ok(expected.clone())]);
        let deadline =
            FirstTokenDeadline::from_start(std::time::Instant::now(), Duration::from_secs(1));

        assert_eq!(
            deadline.next(&mut stream).await.unwrap(),
            Some(Ok(Bytes::new()))
        );
        assert_eq!(
            deadline.next(&mut stream).await.unwrap(),
            Some(Ok(expected))
        );
    }
}
