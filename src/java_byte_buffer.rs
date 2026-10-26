// Heap ByteBuffer subset. Cloning this value copies a Java reference; array()
// exposes the same canonical mutable allocation, including after later writes.
#[derive(Clone, Debug)]
pub struct JavaNioByteBuffer {
    state: std::sync::Arc<std::sync::Mutex<JavaHeapByteBufferState>>,
}
#[derive(Debug)]
struct JavaHeapByteBufferState {
    bytes: std::sync::Arc<std::sync::Mutex<Vec<i8>>>,
    position: usize,
    capacity: usize,
}
impl JavaNioByteBuffer {
    pub fn allocate(capacity: i32) -> Result<Self, JavaException> {
        if capacity < 0 {
            return Err(JavaException::new(
                "IllegalArgumentException",
                format!("capacity < 0: ({} < 0)", capacity),
            ));
        }
        Ok(Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(JavaHeapByteBufferState {
                bytes: std::sync::Arc::new(std::sync::Mutex::new(vec![0; capacity as usize])),
                position: 0,
                capacity: capacity as usize,
            })),
        })
    }
    fn put_octets(&self, bytes: &[u8]) -> Result<Self, JavaException> {
        let mut state = self.state.lock().expect("Java byte buffer mutex poisoned");
        let end = state
            .position
            .checked_add(bytes.len())
            .filter(|end| *end <= state.capacity)
            .ok_or_else(|| {
                JavaException::from_constructor("java.nio.BufferOverflowException", None, None)
            })?;
        {
            let mut array = state.bytes.lock().expect("Java byte array mutex poisoned");
            for (target, byte) in array[state.position..end].iter_mut().zip(bytes) {
                *target = *byte as i8;
            }
        }
        state.position = end;
        Ok(self.clone())
    }
    pub fn put_int(&self, value: i32) -> Result<Self, JavaException> {
        self.put_octets(&value.to_be_bytes())
    }
    pub fn put_long(&self, value: i64) -> Result<Self, JavaException> {
        self.put_octets(&value.to_be_bytes())
    }
    pub fn array(&self) -> std::sync::Arc<std::sync::Mutex<Vec<i8>>> {
        <std::sync::Arc<_> as std::clone::Clone>::clone(
            &self
                .state
                .lock()
                .expect("Java byte buffer mutex poisoned")
                .bytes,
        )
    }
    pub fn capacity(&self) -> i32 {
        self.state
            .lock()
            .expect("Java byte buffer mutex poisoned")
            .capacity as i32
    }
    pub fn position(&self) -> i32 {
        self.state
            .lock()
            .expect("Java byte buffer mutex poisoned")
            .position as i32
    }
    pub fn remaining(&self) -> i32 {
        let state = self.state.lock().expect("Java byte buffer mutex poisoned");
        (state.capacity - state.position) as i32
    }
    pub fn has_remaining(&self) -> bool {
        self.remaining() != 0
    }
}
impl std::fmt::Display for JavaNioByteBuffer {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state = self.state.lock().expect("Java byte buffer mutex poisoned");
        write!(
            out,
            "java.nio.HeapByteBuffer[pos={} lim={} cap={}]",
            state.position, state.capacity, state.capacity
        )
    }
}

pub type ByteBuffer = JavaNioByteBuffer;
impl JavaReferenceIdentity for JavaNioByteBuffer {
    fn java_reference_identity(&self) -> *const () {
        std::sync::Arc::as_ptr(&self.state) as *const ()
    }
}
