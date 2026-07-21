package idiom.gaps;

import native.runtime.NativeArray;
import native.runtime.NativeConsumer;

class FfiOwnedHandleTransfer {
    void consumeBorrowed(final NativeArray array, final NativeConsumer consumer) {
        consumer.consume(array);
        array.close();
    }
}
