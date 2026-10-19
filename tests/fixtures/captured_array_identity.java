import java.util.function.IntSupplier;
import java.util.function.Supplier;

class CapturedArrayIdentity {
    static IntSupplier reader(int[] values) { return () -> values[0]; }
    static int[] identity(int[] values) { return values; }

    static int parameterAlias() {
        int[] source = new int[] { 1 };
        IntSupplier read = reader(source);
        source[0] = 7;
        return read.getAsInt() + source.length;
    }

    static int returnedAlias() {
        int[] source = new int[] { 1 };
        int[] alias = identity(source);
        IntSupplier read = () -> alias[0];
        source[0] = 7;
        alias[0] = 9;
        return read.getAsInt() + source[0];
    }

    static int directAlias() {
        int[] source = new int[] { 1 };
        int[] alias = source;
        IntSupplier read = () -> alias[0];
        source[0] = 7;
        return read.getAsInt();
    }

    static int reboundAlias() {
        int[] source = new int[] { 1 };
        int[] alias = source;
        IntSupplier read = () -> alias[0];
        source = new int[] { 9 };
        alias[0] = 7;
        return read.getAsInt() + source[0];
    }

    static int referenceElements() {
        String[] values = new String[] { "a" };
        IntSupplier read = () -> values[0].length();
        values[0] = "abc";
        return read.getAsInt();
    }

    static int nullElement() {
        String[] values = new String[] { "a" };
        IntSupplier read = () -> values[0].length();
        values[0] = null;
        try { return read.getAsInt(); }
        catch (NullPointerException error) { return -6; }
    }

    static int writes() {
        int[] values = new int[] { 1 };
        Runnable write = () -> { values[0] = values[0] + 1; };
        values[0] = 4;
        write.run();
        return values[0];
    }

    static int shadowed() {
        int result = 0;
        {
            int[] values = new int[] { 2 };
            IntSupplier read = () -> values[0];
            values[0] = 3;
            result += read.getAsInt();
        }
        { int[] values = new int[] { 4 }; result += values[0]; }
        return result;
    }

    static int nested() {
        int[] values = new int[] { 1 };
        Supplier<IntSupplier> factory = () -> () -> values[0];
        IntSupplier read = factory.get();
        values[0] = 8;
        return read.getAsInt();
    }

    static int renamedCapture() {
        int[] type = new int[] { 1 };
        IntSupplier read = () -> type[0];
        type[0] = 9;
        return read.getAsInt();
    }

    static int nullCaught() {
        int[] values = null;
        IntSupplier read = () -> values.length;
        try { return read.getAsInt(); }
        catch (NullPointerException error) { return -4; }
    }

    static int negativeSize() {
        try {
            int[] values = new int[-1];
            IntSupplier read = () -> values.length;
            return read.getAsInt();
        } catch (NegativeArraySizeException error) { return -5; }
    }
}
