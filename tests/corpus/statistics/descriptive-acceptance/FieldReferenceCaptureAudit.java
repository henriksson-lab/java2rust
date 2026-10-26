import java.util.function.IntUnaryOperator;

public class FieldReferenceCaptureAudit {
    interface Input { int apply(int value); }

    static class Holder {
        IntUnaryOperator nativeFunction;
        Input sourceFunction;

        Holder(IntUnaryOperator n, Input s) {
            nativeFunction = n;
            sourceFunction = s;
        }

        Input nativeCapture() { return nativeFunction::applyAsInt; }
        Input sourceCapture() { return sourceFunction::apply; }
        Input nativeLambda() { return value -> nativeFunction.applyAsInt(value); }
        Input sourceLambda() { return value -> sourceFunction.apply(value); }

        Holder replace(IntUnaryOperator n, Input s) {
            nativeFunction = n;
            sourceFunction = s;
            return this;
        }
    }

    public static int run() {
        Holder holder = new Holder(value -> value + 1, value -> value + 2);
        Input nativeSaved = holder.nativeCapture();
        Input sourceSaved = holder.sourceCapture();
        Input nativeLive = holder.nativeLambda();
        Input sourceLive = holder.sourceLambda();
        holder.replace(value -> value + 10, value -> value + 20);
        if (nativeSaved.apply(3) != 4 || sourceSaved.apply(3) != 5) return 1;
        if (holder.nativeCapture().apply(3) != 13 || holder.sourceCapture().apply(3) != 23) return 2;
        if (nativeLive.apply(3) != 13 || sourceLive.apply(3) != 23) return 6;
        holder.replace(null, null);
        if (nativeSaved.apply(3) != 4 || sourceSaved.apply(3) != 5) return 3;
        try { holder.nativeCapture(); return 4; }
        catch (NullPointerException expected) { }
        try { holder.sourceCapture(); return 5; }
        catch (NullPointerException expected) { }
        Input delayed = holder.nativeLambda();
        try { delayed.apply(3); return 7; }
        catch (NullPointerException expected) { }
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
