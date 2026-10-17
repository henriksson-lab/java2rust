use new_java2rust::translate_source;

#[path = "common/mod.rs"]
mod common;
use common::{GeneratedCrateModule, cargo_test_generated_modules};

#[test]
fn output_stream_wrappers_share_memory_and_file_sinks_at_runtime() {
    let translation = translate_source(
        r#"
import java.io.BufferedWriter;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.OutputStreamWriter;

class OutputCarrierRuntime {
    static byte[] snapshot(ByteArrayOutputStream bytes) {
        return bytes.toByteArray();
    }

    static void writeFile(File file) throws Exception {
        OutputStream stream = new FileOutputStream(file);
        BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(stream), 32);
        writer.write("@r\nAC\n+\n!!\n");
        writer.flush();
        writer.close();
    }
}

"#,
    );

    assert!(
        translation.rust.contains("pub struct OutputStream")
            && translation
                .rust
                .contains("impl From<FileOutputStream> for OutputStream")
            && translation
                .rust
                .contains("impl From<ByteArrayOutputStream> for OutputStream")
            && translation.rust.contains("pub struct BufferedWriter")
            && !translation
                .rust
                .contains("unsupported Java external upcast: FileOutputStream -> OutputStream")
            && !translation.rust.contains(
                "unsupported Java external upcast: ByteArrayOutputStream -> OutputStream"
            ),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "output_stream_shared_sink_runtime",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    const FASTQ: &[u8] = b"@r\nAC\n+\n!!\n";

    #[test]
    fn wrappers_retain_the_memory_sink() {
        let bytes = ByteArrayOutputStream::new();
        let stream: OutputStream = bytes.clone().into();
        let writer = BufferedWriter::new(OutputStreamWriter::new(stream), 32);
        writer.write("@r\nAC\n+\n!!\n").unwrap();
        writer.flush().unwrap();
        writer.close().unwrap();
        let observed = OutputCarrierRuntime::snapshot(&bytes);
        assert_eq!(
            observed.into_iter().map(|byte| byte as u8).collect::<Vec<_>>(),
            FASTQ
        );
    }

    #[test]
    fn wrappers_retain_and_close_the_file_sink() {
        let path = std::env::temp_dir().join(format!(
            "new_java2rust_output_stream_{}_{}.fastq",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        OutputCarrierRuntime::write_file(File::new(path.to_string_lossy().into_owned())).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), FASTQ);
        std::fs::remove_file(path).unwrap();
    }
}
"#,
        }],
    );
}

#[test]
fn output_adapters_charset_decode_and_shared_input_dispatch_compile_and_run() {
    let translation = translate_source(
        r#"
import java.io.BufferedOutputStream;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import org.itadaki.bzip2.BZip2OutputStream;

class SharedReadStream extends InputStream {
    int value = 1;

    public int read() { return value++; }

    public int read(byte[] bytes) {
        bytes[0] = (byte) value;
        value = value + 1;
        return 1;
    }

    public int read(byte[] bytes, int offset, int length) {
        if (length == 0) return 0;
        bytes[offset] = (byte) value;
        value = value + 1;
        return 1;
    }
}

class IoAdapterProbe {
    static OutputStream bzip(OutputStream output) throws Exception {
        return new BZip2OutputStream(output);
    }

    static java.io.OutputStream buffered(OutputStream output) {
        return new BufferedOutputStream(output);
    }

    static String decode(ByteArrayOutputStream output) throws Exception {
        return output.toString("UTF-8");
    }

    static int reads() throws Exception {
        SharedReadStream source = new SharedReadStream();
        InputStream input = source;
        int a = input.read();
        int b = input.read();
        return a * 10 + b;
    }
}
"#,
    );

    assert!(
        translation
            .rust
            .contains("BZip2OutputStream> for OutputStream")
            && translation
                .rust
                .contains("impl From<BufferedOutputStream> for JavaIoOutputStream")
            && translation.rust.contains(
                "impl JavaInputStreamDispatch for std::rc::Rc<std::cell::RefCell<SharedReadStream>>"
            )
            && translation
                .rust
                .contains("SharedReadStream::read_with_outer(self.clone())")
            && translation
                .rust
                .contains("SharedReadStream::read_byte_array_with_outer(self.clone(), buffer)")
            && translation.rust.contains(
                "SharedReadStream::read_3_with_outer(self.clone(), buffer, offset, length)"
            ),
        "{}",
        translation.rust
    );

    cargo_test_generated_modules(
        "io_adapter_and_shared_dispatch_runtime",
        vec![GeneratedCrateModule {
            name: "fixture",
            rust: translation.rust,
            tests: r#"
#[cfg(test)]
mod generated_tests {
    use super::*;

    #[test]
    fn charset_overload_decodes_the_shared_memory_sink() {
        let output = ByteArrayOutputStream::new();
        output.write_1(&[104, 105]).unwrap();
        assert_eq!(IoAdapterProbe::decode(&output).unwrap(), "hi".to_string());
    }

    #[test]
    fn all_input_overloads_dispatch_through_the_shared_handle() {
        assert_eq!(IoAdapterProbe::reads().unwrap(), 12);
    }
}
"#,
        }],
    );
}
