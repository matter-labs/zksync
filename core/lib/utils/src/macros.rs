/// Returns future which retries given `Option` with delay and optional timeout
/// until it resolves to `Some(_)` or timeout expires.
#[macro_export]
macro_rules! retry_opt {
    ($fut: expr, $err: expr, $delay: expr) => {
        async {
            loop {
                if let Some(val) = $fut {
                    break val;
                } else {
                    $err;
                    tokio::time::delay_for($delay.into()).await;
                }
            }
        }
    };
    ($fut: expr, $err: expr, $delay: expr, $timeout: expr) => {
        tokio::time::timeout($timeout, $crate::retry_opt!($fut, $err, $delay))
    };
}

#[cfg(test)]
mod tests {
    use futures::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use std::time::Duration;

    struct CondRespond {
        respond: bool,
    }

    impl Future for CondRespond {
        type Output = Option<()>;

        fn poll(self: Pin<&mut Self>, _ctx: &mut Context) -> Poll<Self::Output> {
            if self.respond {
                Poll::Ready(Some(()))
            } else {
                Poll::Ready(None)
            }
        }
    }

    #[tokio::test]
    async fn retries_given_fut() {
        let mut counter = 0;
        let _ = retry_opt! {
            CondRespond {
                respond: counter == 10
            }.await,
            counter += 1,
            Duration::from_millis(10)
        }
        .await;

        assert_eq!(counter, 10);
    }

    #[tokio::test]
    async fn resolves_after_timeout() {
        let mut err_count = 0;
        let val = retry_opt! {
            None::<u8>,
            err_count += 1,
            Duration::from_millis(10),
            Duration::from_millis(100)
        }
        .await;

        assert!((9..11).contains(&err_count));
        assert!(val.is_err());
    }
}
