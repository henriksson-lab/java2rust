package nativeclassassignability;
import java.util.function.DoubleConsumer;
public class NativeClassBase implements DoubleConsumer {
    public void accept(double value) {}
}
class NativeClassLeaf extends NativeClassBase {}
class NativeClassSibling extends NativeClassBase {}
