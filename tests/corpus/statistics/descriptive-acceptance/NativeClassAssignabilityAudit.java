package nativeclassassignability;
import java.util.function.DoubleConsumer;
import java.util.function.IntConsumer;
import java.util.function.Function;
import java.util.function.UnaryOperator;
import java.util.function.BiFunction;
import java.util.function.BinaryOperator;
public class NativeClassAssignabilityAudit {
    interface LocalTag {}
    public static int run() {
        if (!NativeClassBase.class.isAssignableFrom(NativeClassLeaf.class)) return 1;
        if (NativeClassLeaf.class.isAssignableFrom(NativeClassBase.class)) return 2;
        if (NativeClassLeaf.class.isAssignableFrom(NativeClassSibling.class)) return 3;
        if (!DoubleConsumer.class.isAssignableFrom(NativeClassLeaf.class)
                || IntConsumer.class.isAssignableFrom(NativeClassLeaf.class)) return 4;
        if (!Object.class.isAssignableFrom(DoubleConsumer.class)
                || DoubleConsumer.class.isAssignableFrom(IntConsumer.class)) return 5;
        if (!Function.class.isAssignableFrom(UnaryOperator.class)
                || UnaryOperator.class.isAssignableFrom(Function.class)) return 6;
        if (!BiFunction.class.isAssignableFrom(BinaryOperator.class)
                || BinaryOperator.class.isAssignableFrom(BiFunction.class)) return 7;
        if (UnaryOperator.class.isAssignableFrom(BinaryOperator.class)) return 8;
        if (!Function[].class.isAssignableFrom(UnaryOperator[].class)
                || UnaryOperator[].class.isAssignableFrom(Function[].class)) return 9;
        if (!DoubleConsumer.class.getName().equals("java.util.function.DoubleConsumer")) return 10;
        NativeClassBase value = new NativeClassBase();
        if (NativeClassLeaf.class.isAssignableFrom(value.getClass())) return 11;
        if (LocalTag.class.isAssignableFrom(NativeClassBase.class)) return 12;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
