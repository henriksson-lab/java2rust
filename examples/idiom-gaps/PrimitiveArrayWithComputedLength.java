package idiom.gaps;

import java.util.Arrays;

public class PrimitiveArrayWithComputedLength {
    public byte[] filled(int len) {
        byte[] values = new byte[len];
        Arrays.fill(values, (byte) 1);
        return values;
    }
}
