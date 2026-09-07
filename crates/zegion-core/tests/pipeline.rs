use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use aisdk::core::language_model::{
    LanguageModel, LanguageModelOptions, LanguageModelResponse,
};
use zegion_core::pipeline::{Pipeline, ResponseCache, RetryPolicy};

#[derive(Debug, Clone)]
struct FakeModel {
    calls: Arc<AtomicUsize>,
    fail_times: usize,
}

#[async_trait::async_trait]
impl LanguageModel for FakeModel {
    fn name(&self) -> String {
        "fake".into()
    }
    async fn generate_text(
        &mut self,
        options: LanguageModelOptions,
    ) -> aisdk::Result<LanguageModelResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let n = self.calls.load(Ordering::SeqCst);
        if n <= self.fail_times {
            return Err(aisdk::Error::Other("transient".into()));
        }
        // Echo the last user message so we can verify cache behaviour.
        let text = options
            .messages()
            .iter()
            .rev()
            .find_map(|m| match m {
                aisdk::core::Message::User(u) => Some(u.content.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "ok-response".into());
        Ok(LanguageModelResponse::new(text))
    }
    async fn stream_text(
        &mut self,
        _options: LanguageModelOptions,
    ) -> aisdk::Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = aisdk::Result<Vec<aisdk::core::language_model::LanguageModelStreamChunk>>> + Send>,
        >,
    > {
        unimplemented!()
    }
}

// Drive a full request through a capture model, returning the resolved options that
// contain the user message (this is exactly what aisdk passes to generate_text).
async fn resolved_options(text: &str) -> LanguageModelOptions {
    use aisdk::core::LanguageModelRequest;
    use std::sync::Mutex as StdMutex;

    let captured = Arc::new(StdMutex::new(None));
    let captured2 = captured.clone();

    #[derive(Debug, Clone)]
    struct Capture {
        seen: Arc<StdMutex<Option<LanguageModelOptions>>>,
    }
    impl aisdk::core::capabilities::TextInputSupport for Capture {}
    impl aisdk::core::capabilities::ToolCallSupport for Capture {}
    impl aisdk::core::capabilities::StructuredOutputSupport for Capture {}
    #[async_trait::async_trait]
    impl LanguageModel for Capture {
        fn name(&self) -> String {
            "capture".into()
        }
        async fn generate_text(
            &mut self,
            options: LanguageModelOptions,
        ) -> aisdk::Result<LanguageModelResponse> {
            *self.seen.lock().unwrap() = Some(options);
            Ok(LanguageModelResponse::new("x"))
        }
        async fn stream_text(
            &mut self,
            _o: LanguageModelOptions,
        ) -> aisdk::Result<
            std::pin::Pin<
                Box<dyn futures::Stream<Item = aisdk::Result<Vec<aisdk::core::language_model::LanguageModelStreamChunk>>> + Send>,
            >,
        > {
            unimplemented!()
        }
    }

    let capture = Capture { seen: captured2 };
    let mut req = LanguageModelRequest::builder()
        .model(capture)
        .system("sys")
        .prompt(text)
        .build();
    let _ = req.generate_text().await;
    let result = captured.lock().unwrap().clone();
    result.expect("captured options")
}

#[tokio::test(flavor = "multi_thread")]
async fn pipeline_retries_until_success() {
    let calls = Arc::new(AtomicUsize::new(0));
    let model = FakeModel {
        calls: calls.clone(),
        fail_times: 2,
    };
    let pipeline = Pipeline::new(model).with_retry(RetryPolicy {
        max_attempts: 4,
        initial_backoff: Duration::from_millis(1),
    });
    let options = resolved_options("hello").await;
    let resp = pipeline
        .generate_optimized(options)
        .await
        .expect("succeeds after retries");
    assert!(response_text(&resp).contains("hello"));
    assert_eq!(calls.load(Ordering::SeqCst), 3, "2 failures + 1 success");
}

#[tokio::test(flavor = "multi_thread")]
async fn pipeline_caches_identical_requests() {
    let calls = Arc::new(AtomicUsize::new(0));
    let model = FakeModel {
        calls: calls.clone(),
        fail_times: 0,
    };
    let cache = Arc::new(ResponseCache::new(Duration::from_secs(60), 16));
    let pipeline = Pipeline::new(model).with_cache(cache);

    let a = resolved_options("same question").await;
    let b = resolved_options("same question").await;
    let r1 = pipeline.generate_optimized(a).await.unwrap();
    let r2 = pipeline.generate_optimized(b).await.unwrap();

    assert_eq!(response_text(&r1), response_text(&r2));
    assert_eq!(calls.load(Ordering::SeqCst), 1, "second identical call should hit cache");
}

#[tokio::test(flavor = "multi_thread")]
async fn pipeline_guardrail_blocks_secret_input() {
    let model = FakeModel {
        calls: Arc::new(AtomicUsize::new(0)),
        fail_times: 0,
    };
    let pipeline = Pipeline::new(model)
        .with_guardrails(Arc::new(zegion_core::guardrails::default_guardrails()));
    let options = resolved_options("here is my -----BEGIN PRIVATE KEY----- data").await;
    let result = pipeline.generate_optimized(options).await;
    assert!(result.is_err(), "guardrail should block secret input");
}

fn response_text(r: &LanguageModelResponse) -> String {
    r.contents
        .iter()
        .find_map(|c| match c {
            aisdk::core::language_model::LanguageModelResponseContentType::Text(t) => {
                Some(t.clone())
            }
            _ => None,
        })
        .unwrap_or_default()
}
