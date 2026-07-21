package idiom.gaps;

import native.runtime.NativeArray;
import native.runtime.NativeLibrary;
import native.runtime.NativeQuery;

class FfiNativeHandoverCall {
    int count(final String workspace, final String interval) {
        final NativeArray array = NativeLibrary.openArray(workspace);
        try {
            final NativeQuery query = array.query(interval);
            return query.count();
        } finally {
            array.close();
        }
    }
}
