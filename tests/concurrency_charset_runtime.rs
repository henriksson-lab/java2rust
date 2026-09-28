use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn charset_constants_and_count_down_latch_are_stateful_at_runtime() {
    let translation = translate_source(
        r#"
import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.CountDownLatch;

class PlatformRuntimeUse {
    static String utf8Name() {
        Charset charset = StandardCharsets.UTF_8;
        return charset.name();
    }

    static void waitFor(CountDownLatch latch) throws InterruptedException {
        latch.await();
    }

    static void release(CountDownLatch latch) {
        latch.countDown();
    }

    static long remaining(CountDownLatch latch) {
        return latch.getCount();
    }
}
"#,
    );

    assert!(
        translation.rust.contains("pub struct Charset")
            && translation
                .rust
                .contains("pub type StandardCharsets = JavaNioCharsetStandardCharsets")
            && translation
                .rust
                .contains("pub const UTF_8: Charset = Charset::new(\"UTF-8\")")
            && translation.rust.contains("pub struct CountDownLatch")
            && translation.rust.contains("pub fn count_down(&self)")
            && translation
                .rust
                .contains("pub fn r#await(&self) -> Result<(), JavaException>")
            && !translation
                .rust
                .contains("CountDownLatch.await is not modelled"),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "charset_and_latch_runtime",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn standard_charset_constants_keep_canonical_names() {
        assert_eq!(PlatformRuntimeUse::utf8_name(), "UTF-8");
        assert_eq!(StandardCharsets::US_ASCII.name(), "US-ASCII");
        assert_eq!(StandardCharsets::ISO_8859_1.name(), "ISO-8859-1");
        assert_eq!(StandardCharsets::UTF_8.name(), "UTF-8");
        assert_eq!(StandardCharsets::UTF_16BE.name(), "UTF-16BE");
        assert_eq!(StandardCharsets::UTF_16LE.name(), "UTF-16LE");
        assert_eq!(StandardCharsets::UTF_16.name(), "UTF-16");
    }

    #[test]
    fn cloned_latches_share_count_and_wake_waiters() {
        let latch = CountDownLatch::new(2_i64);
        assert_eq!(PlatformRuntimeUse::remaining(&latch).unwrap(), 2);
        let waiter_latch = latch.clone();
        let (finished_tx, finished_rx) = std::sync::mpsc::channel();
        let waiter = std::thread::spawn(move || {
            PlatformRuntimeUse::wait_for(&waiter_latch).unwrap();
            finished_tx.send(()).unwrap();
        });

        PlatformRuntimeUse::release(&latch);
        assert_eq!(PlatformRuntimeUse::remaining(&latch).unwrap(), 1);
        assert!(finished_rx.recv_timeout(std::time::Duration::from_millis(20)).is_err());

        PlatformRuntimeUse::release(&latch);
        finished_rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
        waiter.join().unwrap();
        assert_eq!(PlatformRuntimeUse::remaining(&latch).unwrap(), 0);

        // Java specifies that extra countDown calls after zero have no effect.
        PlatformRuntimeUse::release(&latch);
        assert_eq!(PlatformRuntimeUse::remaining(&latch).unwrap(), 0);
    }
}
"#,
        }],
    );
}
